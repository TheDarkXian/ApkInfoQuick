
use std::collections::{BTreeSet, HashMap};
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::Reader;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::error::BackendError;
use crate::model::{ApkInfoData, ApkInfoEnvelope, SignerInfo};

pub fn parse_apk_to_envelope(path: &Path) -> ApkInfoEnvelope {
    match parse_apk(path) {
        Ok((data, warnings)) => ApkInfoEnvelope::ok(data, warnings),
        Err((err, warnings)) => ApkInfoEnvelope::err(
            err.code(),
            sanitize_error_message(err.to_string(), path),
            ApkInfoData::placeholder(),
            warnings,
        ),
    }
}

pub fn parse_apk_tauri(file_path: String) -> ApkInfoEnvelope {
    let path = PathBuf::from(file_path);
    parse_apk_to_envelope(&path)
}

fn parse_apk(path: &Path) -> Result<(ApkInfoData, Vec<String>), (BackendError, Vec<String>)> {
    input_validation::validate_input(path).map_err(|e| (e, Vec::new()))?;

    let file = File::open(path).map_err(|_| (BackendError::ApkOpenFailed, Vec::new()))?;
    let mut archive = ZipArchive::new(file).map_err(|_| (BackendError::ApkOpenFailed, Vec::new()))?;

    let mut warnings = Vec::new();

    let manifest_bytes = archive_reader::read_manifest_bytes(&mut archive)
        .map_err(|e| (e, warnings.clone()))?;

    let mut manifest = manifest_reader::parse_manifest(&manifest_bytes, &mut warnings)
        .map_err(|e| (e, warnings.clone()))?;

    resource_resolver::resolve_app_name(&mut manifest, &mut archive, &mut warnings);

    let abis = archive_reader::infer_abis(&mut archive);
    let channel = channel_resolver::resolve(&manifest, path, &mut archive, &mut warnings);
    let icon_url = icon_extractor::extract_best_icon(path, &manifest, &mut archive, &mut warnings);
    let signers = signer_reader::extract_signers(path, &mut archive, &mut warnings);

    let data = envelope_mapper::map_to_data(path, manifest, abis, channel, icon_url, signers);
    Ok((data, warnings))
}

mod warnings {
    pub const CHANNEL_NOT_FOUND: &str = "CHANNEL_NOT_FOUND";
    pub const ICON_NOT_FOUND: &str = "ICON_NOT_FOUND";
    pub const APP_NAME_UNRESOLVED: &str = "APP_NAME_UNRESOLVED";
    pub const SIGNATURE_PARTIAL: &str = "SIGNATURE_PARTIAL";
    pub const SIGNATURE_BLOCK_DETECTED_UNPARSED: &str = "SIGNATURE_BLOCK_DETECTED_UNPARSED";
    pub const MANIFEST_BINARY_PARTIAL: &str = "MANIFEST_BINARY_PARTIAL";
}

fn push_warning(warnings: &mut Vec<String>, code: &str) {
    if !warnings.iter().any(|w| w == code) {
        warnings.push(code.to_string());
    }
}

mod input_validation {
    use super::*;

    pub fn validate_input(path: &Path) -> Result<(), BackendError> {
        if !path.exists() {
            return Err(BackendError::InputNotFound);
        }

        if !path.is_file() {
            return Err(BackendError::InputNotFile);
        }

        let is_apk = path
            .extension()
            .and_then(OsStr::to_str)
            .map(|ext| ext.eq_ignore_ascii_case("apk"))
            .unwrap_or(false);

        if !is_apk {
            return Err(BackendError::InputNotApk);
        }

        Ok(())
    }
}

mod archive_reader {
    use super::*;

    pub fn read_manifest_bytes(archive: &mut ZipArchive<File>) -> Result<Vec<u8>, BackendError> {
        let mut entry = archive
            .by_name("AndroidManifest.xml")
            .map_err(|_| BackendError::ManifestNotFound)?;

        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .map_err(|_| BackendError::ApkEntryReadFailed)?;

        if buf.is_empty() {
            return Err(BackendError::ManifestParseFailed);
        }

        Ok(buf)
    }

    pub fn infer_abis(archive: &mut ZipArchive<File>) -> Vec<String> {
        let known = ["arm64-v8a", "armeabi-v7a", "x86", "x86_64"];
        let mut set = BTreeSet::new();

        for i in 0..archive.len() {
            if let Ok(file) = archive.by_index(i) {
                let name = file.name();
                for abi in known {
                    let prefix = format!("lib/{abi}/");
                    if name.starts_with(&prefix) {
                        set.insert(abi.to_string());
                    }
                }
            }
        }

        set.into_iter().collect()
    }

    pub fn list_entries(archive: &mut ZipArchive<File>) -> Vec<String> {
        let mut names = Vec::new();
        for i in 0..archive.len() {
            if let Ok(file) = archive.by_index(i) {
                names.push(file.name().to_string());
            }
        }
        names
    }
}

#[derive(Debug, Default, Clone)]
struct ManifestInfo {
    package_name: Option<String>,
    app_name: Option<String>,
    app_icon: Option<String>,
    min_sdk_version: Option<i32>,
    target_sdk_version: Option<i32>,
    compile_sdk_version: Option<i32>,
    version_code: Option<i64>,
    version_name: Option<String>,
    permissions: Vec<String>,
    meta_data: HashMap<String, String>,
}
mod manifest_reader {
    use super::*;

    const RES_XML_TYPE: u16 = 0x0003;
    const RES_STRING_POOL_TYPE: u16 = 0x0001;
    const RES_XML_START_ELEMENT_TYPE: u16 = 0x0102;

    const TYPE_REFERENCE: u8 = 0x01;
    const TYPE_STRING: u8 = 0x03;
    const TYPE_INT_DEC: u8 = 0x10;
    const TYPE_INT_HEX: u8 = 0x11;
    const TYPE_INT_BOOLEAN: u8 = 0x12;

    const NO_INDEX: u32 = 0xffff_ffff;

    pub fn parse_manifest(bytes: &[u8], warnings: &mut Vec<String>) -> Result<ManifestInfo, BackendError> {
        if bytes.first().copied() == Some(b'<') {
            return parse_text_manifest(bytes);
        }

        let parsed = parse_binary_manifest(bytes, warnings)?;
        Ok(parsed)
    }

    fn parse_text_manifest(bytes: &[u8]) -> Result<ManifestInfo, BackendError> {
        let mut reader = Reader::from_reader(bytes);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();
        let mut info = ManifestInfo::default();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let attrs = parse_text_attrs(&e, &reader)?;
                    apply_tag(&tag, &attrs, &mut info);
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(_) => return Err(BackendError::ManifestParseFailed),
            }
            buf.clear();
        }

        Ok(info)
    }

    fn parse_text_attrs(
        event: &quick_xml::events::BytesStart<'_>,
        reader: &Reader<&[u8]>,
    ) -> Result<HashMap<String, String>, BackendError> {
        let mut attrs = HashMap::new();
        for attr in event.attributes() {
            let attr = attr.map_err(|_| BackendError::ManifestParseFailed)?;
            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
            let value = attr
                .decode_and_unescape_value(reader.decoder())
                .map_err(|_| BackendError::ManifestParseFailed)?
                .to_string();
            attrs.insert(strip_namespace(&key).to_string(), value);
        }
        Ok(attrs)
    }

    fn parse_binary_manifest(bytes: &[u8], warnings: &mut Vec<String>) -> Result<ManifestInfo, BackendError> {
        if bytes.len() < 8 {
            return Err(BackendError::ManifestParseFailed);
        }

        let xml_type = le_u16(bytes, 0)?;
        if xml_type != RES_XML_TYPE {
            return Err(BackendError::ManifestParseFailed);
        }

        let total_size = le_u32(bytes, 4)? as usize;
        if total_size > bytes.len() {
            return Err(BackendError::ManifestParseFailed);
        }

        let mut strings: Vec<String> = Vec::new();
        let mut info = ManifestInfo::default();
        let mut offset = 8;

        while offset + 8 <= total_size {
            let chunk_type = le_u16(bytes, offset)?;
            let header_size = le_u16(bytes, offset + 2)? as usize;
            let chunk_size = le_u32(bytes, offset + 4)? as usize;

            if chunk_size == 0 || offset + chunk_size > total_size || header_size > chunk_size {
                return Err(BackendError::ManifestParseFailed);
            }

            match chunk_type {
                RES_STRING_POOL_TYPE => {
                    strings = parse_string_pool(&bytes[offset..offset + chunk_size])?;
                }
                RES_XML_START_ELEMENT_TYPE => {
                    parse_binary_start_element(&bytes[offset..offset + chunk_size], &strings, &mut info)?;
                }
                _ => {}
            }

            offset += chunk_size;
        }

        let is_partial = info.package_name.is_none()
            || info.min_sdk_version.is_none()
            || info.target_sdk_version.is_none();
        if is_partial {
            super::push_warning(warnings, super::warnings::MANIFEST_BINARY_PARTIAL);
        }
        Ok(info)
    }

    fn parse_binary_start_element(
        chunk: &[u8],
        strings: &[String],
        info: &mut ManifestInfo,
    ) -> Result<(), BackendError> {
        if chunk.len() < 36 {
            return Err(BackendError::ManifestParseFailed);
        }

        let header_size = le_u16(chunk, 2)? as usize;
        if header_size < 16 || chunk.len() < header_size + 20 {
            return Err(BackendError::ManifestParseFailed);
        }

        let ext = header_size;
        let name_idx = le_u32(chunk, ext + 4)?;
        let attribute_start = le_u16(chunk, ext + 8)? as usize;
        let attribute_size = le_u16(chunk, ext + 10)? as usize;
        let attribute_count = le_u16(chunk, ext + 12)? as usize;

        if attribute_size < 20 {
            return Err(BackendError::ManifestParseFailed);
        }

        let element_name = get_string(strings, name_idx).unwrap_or_default();
        let attrs_start = ext + attribute_start;

        let mut attrs = HashMap::new();

        for i in 0..attribute_count {
            let base = attrs_start + i * attribute_size;
            if base + 20 > chunk.len() {
                return Err(BackendError::ManifestParseFailed);
            }

            let attr_name_idx = le_u32(chunk, base + 4)?;
            let raw_value_idx = le_u32(chunk, base + 8)?;
            let data_type = chunk[base + 15];
            let data = le_u32(chunk, base + 16)?;

            let name = get_string(strings, attr_name_idx).unwrap_or_default();
            let value = if raw_value_idx != NO_INDEX {
                get_string(strings, raw_value_idx).unwrap_or_default()
            } else {
                typed_value_to_string(strings, data_type, data)
            };

            attrs.insert(name, value);
        }

        apply_tag(&element_name, &attrs, info);
        Ok(())
    }

    fn apply_tag(tag: &str, attrs: &HashMap<String, String>, info: &mut ManifestInfo) {
        match tag {
            "manifest" => {
                if let Some(v) = attrs.get("package") {
                    info.package_name = Some(v.clone());
                }
                if let Some(v) = attrs.get("versionCode") {
                    info.version_code = parse_i64(v);
                }
                if let Some(v) = attrs.get("versionName") {
                    info.version_name = Some(v.clone());
                }
                if let Some(v) = attrs.get("compileSdkVersion") {
                    info.compile_sdk_version = parse_i32(v);
                }
            }
            "uses-sdk" => {
                if let Some(v) = attrs.get("minSdkVersion") {
                    info.min_sdk_version = parse_i32(v);
                }
                if let Some(v) = attrs.get("targetSdkVersion") {
                    info.target_sdk_version = parse_i32(v);
                }
            }
            "uses-permission" => {
                if let Some(v) = attrs.get("name") {
                    info.permissions.push(v.clone());
                }
            }
            "application" => {
                if let Some(v) = attrs.get("label") {
                    info.app_name = Some(v.clone());
                }
                if let Some(v) = attrs.get("icon") {
                    info.app_icon = Some(v.clone());
                }
            }
            "meta-data" => {
                if let Some(name) = attrs.get("name") {
                    if let Some(value) = attrs.get("value") {
                        info.meta_data.insert(name.clone(), value.clone());
                    }
                }
            }
            _ => {}
        }
    }

    fn parse_string_pool(chunk: &[u8]) -> Result<Vec<String>, BackendError> {
        if chunk.len() < 28 {
            return Err(BackendError::ManifestParseFailed);
        }

        let string_count = le_u32(chunk, 8)? as usize;
        let flags = le_u32(chunk, 16)?;
        let strings_start = le_u32(chunk, 20)? as usize;
        let utf8 = (flags & 0x0000_0100) != 0;

        let index_table_start = 28;
        let index_table_end = index_table_start + string_count * 4;
        if index_table_end > chunk.len() {
            return Err(BackendError::ManifestParseFailed);
        }

        let mut offsets = Vec::with_capacity(string_count);
        for i in 0..string_count {
            offsets.push(le_u32(chunk, index_table_start + i * 4)? as usize);
        }

        let mut out = Vec::with_capacity(string_count);
        for off in offsets {
            let start = strings_start + off;
            if start >= chunk.len() {
                return Err(BackendError::ManifestParseFailed);
            }
            let s = if utf8 {
                decode_utf8_pool_string(chunk, start)?
            } else {
                decode_utf16_pool_string(chunk, start)?
            };
            out.push(s);
        }

        Ok(out)
    }

    fn decode_utf8_pool_string(chunk: &[u8], start: usize) -> Result<String, BackendError> {
        let (_, off1) = decode_length8(chunk, start)?;
        let (byte_len, off2) = decode_length8(chunk, off1)?;
        let end = off2 + byte_len;
        if end > chunk.len() {
            return Err(BackendError::ManifestParseFailed);
        }
        let data = &chunk[off2..end];
        Ok(String::from_utf8_lossy(data).to_string())
    }

    fn decode_utf16_pool_string(chunk: &[u8], start: usize) -> Result<String, BackendError> {
        let (char_len, mut off) = decode_length16(chunk, start)?;
        let mut vals = Vec::with_capacity(char_len);
        for _ in 0..char_len {
            if off + 2 > chunk.len() {
                return Err(BackendError::ManifestParseFailed);
            }
            let v = u16::from_le_bytes([chunk[off], chunk[off + 1]]);
            vals.push(v);
            off += 2;
        }
        Ok(String::from_utf16_lossy(&vals))
    }

    fn decode_length8(chunk: &[u8], start: usize) -> Result<(usize, usize), BackendError> {
        if start >= chunk.len() {
            return Err(BackendError::ManifestParseFailed);
        }
        let first = chunk[start];
        if (first & 0x80) == 0 {
            Ok((first as usize, start + 1))
        } else {
            if start + 1 >= chunk.len() {
                return Err(BackendError::ManifestParseFailed);
            }
            let second = chunk[start + 1];
            let len = (((first & 0x7f) as usize) << 8) | second as usize;
            Ok((len, start + 2))
        }
    }

    fn decode_length16(chunk: &[u8], start: usize) -> Result<(usize, usize), BackendError> {
        if start + 2 > chunk.len() {
            return Err(BackendError::ManifestParseFailed);
        }
        let first = u16::from_le_bytes([chunk[start], chunk[start + 1]]);
        if (first & 0x8000) == 0 {
            Ok((first as usize, start + 2))
        } else {
            if start + 4 > chunk.len() {
                return Err(BackendError::ManifestParseFailed);
            }
            let second = u16::from_le_bytes([chunk[start + 2], chunk[start + 3]]);
            let len = (((first & 0x7fff) as usize) << 16) | second as usize;
            Ok((len, start + 4))
        }
    }

    fn typed_value_to_string(strings: &[String], data_type: u8, data: u32) -> String {
        match data_type {
            TYPE_STRING => get_string(strings, data).unwrap_or_default(),
            TYPE_INT_DEC => format!("{}", data as i32),
            TYPE_INT_HEX => format!("0x{data:08x}"),
            TYPE_INT_BOOLEAN => {
                if data == 0 {
                    "false".to_string()
                } else {
                    "true".to_string()
                }
            }
            TYPE_REFERENCE => format!("@0x{data:08x}"),
            _ => format!("0x{data:08x}"),
        }
    }

    fn get_string(strings: &[String], idx: u32) -> Option<String> {
        strings.get(idx as usize).cloned()
    }

    fn parse_i32(s: &str) -> Option<i32> {
        if let Some(hex) = s.strip_prefix("0x") {
            i32::from_str_radix(hex, 16).ok()
        } else {
            s.parse::<i32>().ok()
        }
    }

    fn parse_i64(s: &str) -> Option<i64> {
        if let Some(hex) = s.strip_prefix("0x") {
            i64::from_str_radix(hex, 16).ok()
        } else {
            s.parse::<i64>().ok()
        }
    }

    fn strip_namespace(name: &str) -> &str {
        name.rsplit(':').next().unwrap_or(name)
    }

    fn le_u16(bytes: &[u8], offset: usize) -> Result<u16, BackendError> {
        if offset + 2 > bytes.len() {
            return Err(BackendError::ManifestParseFailed);
        }
        Ok(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]))
    }

    fn le_u32(bytes: &[u8], offset: usize) -> Result<u32, BackendError> {
        if offset + 4 > bytes.len() {
            return Err(BackendError::ManifestParseFailed);
        }
        Ok(u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]))
    }
}

mod resource_resolver {
    use super::*;

    pub fn resolve_app_name(
        manifest: &mut ManifestInfo,
        archive: &mut ZipArchive<File>,
        warnings: &mut Vec<String>,
    ) {
        let Some(label) = manifest.app_name.clone() else {
            return;
        };

        if !label.starts_with('@') {
            return;
        }

        if let Some(key) = extract_string_key(&label) {
            if let Some(v) = read_string_resource(archive, key) {
                manifest.app_name = Some(v);
                return;
            }
        }

        manifest.app_name = None;
        super::push_warning(warnings, super::warnings::APP_NAME_UNRESOLVED);
    }

    fn extract_string_key(label: &str) -> Option<&str> {
        if let Some(key) = label.strip_prefix("@string/") {
            return Some(key);
        }

        if let Some(raw) = label.strip_prefix('@') {
            if let Some((_, key)) = raw.rsplit_once(":string/") {
                return Some(key);
            }
            if let Some(key) = raw.strip_prefix("string/") {
                return Some(key);
            }
        }

        None
    }

    fn read_string_resource(archive: &mut ZipArchive<File>, key: &str) -> Option<String> {
        let mut candidates = vec!["res/values/strings.xml".to_string()];
        for name in archive_reader::list_entries(archive) {
            if name.starts_with("res/values") && name.ends_with("/strings.xml") && name != "res/values/strings.xml" {
                candidates.push(name);
            }
        }

        for path in candidates {
            if let Ok(mut entry) = archive.by_name(&path) {
                let mut content = String::new();
                if entry.read_to_string(&mut content).is_ok() {
                    if let Some(v) = find_string_in_xml(&content, key) {
                        return Some(v);
                    }
                }
            }
        }
        None
    }

    fn find_string_in_xml(xml: &str, key: &str) -> Option<String> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();
        let mut in_target = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    if e.name().as_ref() == b"string" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name" {
                                let v = attr.decode_and_unescape_value(reader.decoder()).ok()?;
                                if v.as_ref() == key {
                                    in_target = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                Ok(Event::Text(e)) => {
                    if in_target {
                        return Some(String::from_utf8_lossy(e.as_ref()).to_string());
                    }
                }
                Ok(Event::End(e)) => {
                    if e.name().as_ref() == b"string" {
                        in_target = false;
                    }
                }
                Ok(Event::Eof) => return None,
                Err(_) => return None,
                _ => {}
            }
            buf.clear();
        }
    }
}
mod icon_extractor {
    use super::*;

    const ICON_DENSITY_PRIORITY: [&str; 10] = [
        "xxxhdpi", "xxhdpi", "xhdpi", "hdpi", "mdpi", "ldpi", "anydpi", "tvdpi", "nodpi", "",
    ];
    const ICON_EXT_PRIORITY: [&str; 2] = ["png", "webp"];

    pub fn extract_best_icon(
        apk_path: &Path,
        manifest: &ManifestInfo,
        archive: &mut ZipArchive<File>,
        warnings: &mut Vec<String>,
    ) -> String {
        let entries = archive_reader::list_entries(archive);

        let candidate_entries = if let Some(icon) = &manifest.app_icon {
            from_manifest_icon(icon, &entries)
        } else {
            Vec::new()
        };

        let mut candidates = candidate_entries;
        if candidates.is_empty() {
            candidates = fallback_icon_candidates(&entries);
        }

        for entry_name in candidates {
            if let Ok(mut entry) = archive.by_name(&entry_name) {
                let mut buf = Vec::new();
                if entry.read_to_end(&mut buf).is_ok() {
                    if let Some(url) = write_icon_to_temp(apk_path, &entry_name, &buf) {
                        return url;
                    }
                }
            }
        }

        super::push_warning(warnings, super::warnings::ICON_NOT_FOUND);
        String::new()
    }

    fn from_manifest_icon(icon: &str, entries: &[String]) -> Vec<String> {
        if !icon.starts_with('@') {
            return vec![icon.to_string()];
        }

        let raw = icon.trim_start_matches('@');
        let parts: Vec<&str> = raw.split('/').collect();
        if parts.len() != 2 {
            return Vec::new();
        }

        let icon_type = parts[0].trim_start_matches("android:");
        let icon_name = parts[1];

        let mut out = Vec::new();
        for density in ICON_DENSITY_PRIORITY {
            let qualifier = if density.is_empty() {
                icon_type.to_string()
            } else {
                format!("{icon_type}-{density}")
            };
            for ext in ICON_EXT_PRIORITY {
                let candidate = format!("res/{qualifier}/{icon_name}.{ext}");
                if entries.iter().any(|e| e == &candidate) {
                    out.push(candidate);
                }
            }
        }

        out
    }

    fn fallback_icon_candidates(entries: &[String]) -> Vec<String> {
        let mut out = Vec::new();
        for density in ICON_DENSITY_PRIORITY {
            for base in ["mipmap", "drawable"] {
                let dir = if density.is_empty() {
                    base.to_string()
                } else {
                    format!("{base}-{density}")
                };
                for name in ["ic_launcher", "app_icon", "icon"] {
                    for ext in ICON_EXT_PRIORITY {
                        let p = format!("res/{dir}/{name}.{ext}");
                        if entries.iter().any(|e| e == &p) {
                            out.push(p.clone());
                        }
                    }
                }
            }
        }
        if out.is_empty() {
            for entry in entries {
                if (entry.starts_with("res/mipmap") || entry.starts_with("res/drawable"))
                    && (entry.ends_with(".png") || entry.ends_with(".webp"))
                {
                    out.push(entry.clone());
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    fn write_icon_to_temp(apk_path: &Path, entry_name: &str, bytes: &[u8]) -> Option<String> {
        let dir = std::env::temp_dir().join("apk-info-icons");
        std::fs::create_dir_all(&dir).ok()?;

        let stem = apk_path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("app")
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
            .collect::<String>();

        let digest = Sha256::digest(entry_name.as_bytes());
        let suffix = format!("{:x}", digest);
        let ext = Path::new(entry_name)
            .extension()
            .and_then(OsStr::to_str)
            .unwrap_or("png");
        let filename = format!("{stem}-{}.{}", &suffix[..12], ext);
        let target = dir.join(filename);
        std::fs::write(&target, bytes).ok()?;

        let normalized = target.to_string_lossy().replace('\\', "/");
        Some(format!("file:///{normalized}"))
    }
}

mod signer_reader {
    use super::*;

    pub fn extract_signers(
        apk_path: &Path,
        archive: &mut ZipArchive<File>,
        warnings: &mut Vec<String>,
    ) -> Vec<SignerInfo> {
        let mut signers = Vec::new();

        let signature_entries = archive_reader::list_entries(archive)
            .into_iter()
            .filter(|name| {
                let upper = name.to_uppercase();
                upper.starts_with("META-INF/")
                    && (upper.ends_with(".RSA") || upper.ends_with(".DSA") || upper.ends_with(".EC"))
            })
            .collect::<Vec<_>>();

        for name in signature_entries {
            if let Ok(mut entry) = archive.by_name(&name) {
                let mut buf = Vec::new();
                if entry.read_to_end(&mut buf).is_ok() {
                    let hash = Sha256::digest(&buf);
                    let digest = format!("{:x}", hash).to_uppercase();
                    let subject = std::path::Path::new(&name)
                        .file_name()
                        .and_then(OsStr::to_str)
                        .unwrap_or("unknown")
                        .to_string();

                    signers.push(SignerInfo {
                        scheme: "v1".to_string(),
                        cert_sha256: digest,
                        issuer: "unknown".to_string(),
                        subject,
                        valid_from: String::new(),
                        valid_to: String::new(),
                    });
                }
            }
        }

        if has_apk_sig_block(apk_path) {
            super::push_warning(warnings, super::warnings::SIGNATURE_BLOCK_DETECTED_UNPARSED);
        }

        if !signers.is_empty() || has_any_signature_hint(archive) {
            super::push_warning(warnings, super::warnings::SIGNATURE_PARTIAL);
        }

        signers
    }

    fn has_any_signature_hint(archive: &mut ZipArchive<File>) -> bool {
        archive_reader::list_entries(archive).into_iter().any(|name| {
            let upper = name.to_uppercase();
            upper.starts_with("META-INF/")
                && (upper.ends_with(".SF")
                    || upper.ends_with(".RSA")
                    || upper.ends_with(".DSA")
                    || upper.ends_with(".EC"))
        })
    }

    fn has_apk_sig_block(apk_path: &Path) -> bool {
        let Ok(mut file) = File::open(apk_path) else {
            return false;
        };

        let Ok(total_len) = file.metadata().map(|m| m.len()) else {
            return false;
        };
        let read_len = total_len.min(256 * 1024);
        if file.seek(SeekFrom::End(-(read_len as i64))).is_err() {
            return false;
        }

        let mut tail = Vec::with_capacity(read_len as usize);
        if file.take(read_len).read_to_end(&mut tail).is_err() {
            return false;
        }

        tail.windows("APK Sig Block 42".len())
            .any(|w| w == b"APK Sig Block 42")
    }
}

mod channel_resolver {
    use super::*;

    const KEYS: [&str; 4] = ["UMENG_CHANNEL", "CHANNEL", "channel", "umeng_channel"];

    pub fn resolve(
        manifest: &ManifestInfo,
        path: &Path,
        archive: &mut ZipArchive<File>,
        warnings: &mut Vec<String>,
    ) -> String {
        if let Some(c) = from_manifest_meta(manifest) {
            return c;
        }

        if let Some(c) = from_archive_file(archive) {
            return c;
        }

        if let Some(c) = from_filename(path) {
            return c;
        }

        super::push_warning(warnings, super::warnings::CHANNEL_NOT_FOUND);
        "unknown".to_string()
    }

    fn from_manifest_meta(manifest: &ManifestInfo) -> Option<String> {
        for key in KEYS {
            if let Some(v) = manifest.meta_data.get(key) {
                if !v.trim().is_empty() {
                    return Some(v.clone());
                }
            }
        }
        None
    }

    fn from_archive_file(archive: &mut ZipArchive<File>) -> Option<String> {
        for name in archive_reader::list_entries(archive) {
            if let Some(raw) = name.strip_prefix("META-INF/channel_") {
                if !raw.is_empty() {
                    return Some(raw.to_string());
                }
            }
        }
        None
    }

    fn from_filename(path: &Path) -> Option<String> {
        let stem = path.file_stem()?.to_string_lossy().to_lowercase();
        let tokens = [
            "huawei",
            "xiaomi",
            "oppo",
            "vivo",
            "tencent",
            "baidu",
            "googleplay",
            "samsung",
        ];
        for token in tokens {
            if stem.contains(token) {
                return Some(token.to_string());
            }
        }
        None
    }
}

mod envelope_mapper {
    use super::*;

    pub fn map_to_data(
        path: &Path,
        manifest: ManifestInfo,
        abis: Vec<String>,
        channel: String,
        icon_url: String,
        signers: Vec<SignerInfo>,
    ) -> ApkInfoData {
        let package_name = manifest
            .package_name
            .unwrap_or_else(|| infer_package_name(path));

        let app_name = manifest.app_name.unwrap_or_else(|| infer_app_name(path));

        ApkInfoData {
            package_name,
            app_name,
            icon_url,
            min_sdk_version: manifest.min_sdk_version.unwrap_or(1),
            target_sdk_version: manifest.target_sdk_version.unwrap_or(1),
            compile_sdk_version: manifest.compile_sdk_version,
            version_code: manifest.version_code.unwrap_or(1),
            version_name: manifest.version_name,
            permissions: dedup(manifest.permissions),
            signers,
            abis,
            channel,
        }
    }

    fn infer_package_name(path: &Path) -> String {
        path.file_stem()
            .and_then(OsStr::to_str)
            .map(|s| format!("local.{}", sanitize(s).to_lowercase()))
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn infer_app_name(path: &Path) -> String {
        path.file_stem()
            .and_then(OsStr::to_str)
            .map(sanitize)
            .unwrap_or_else(|| "Unknown".to_string())
    }

    fn sanitize(raw: &str) -> String {
        raw.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
    }

    fn dedup(items: Vec<String>) -> Vec<String> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for i in items {
            if seen.insert(i.clone()) {
                out.push(i);
            }
        }
        out
    }
}

fn sanitize_error_message(message: String, path: &Path) -> String {
    let path_str = path.to_string_lossy();
    message.replace(path_str.as_ref(), &sanitize_path(path))
}

fn sanitize_path(path: &Path) -> String {
    path.file_name()
        .map(|name| format!("<redacted>/{}", name.to_string_lossy()))
        .unwrap_or_else(|| "<redacted>".to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;

    #[test]
    fn non_apk_returns_error_envelope() {
        let dir = tempdir().expect("create tempdir");
        let not_apk = dir.path().join("demo.txt");
        std::fs::write(&not_apk, "x").expect("write file");
        let envelope = parse_apk_to_envelope(&not_apk);

        assert!(!envelope.success);
        assert_eq!(envelope.error_code.as_deref(), Some("INPUT_NOT_APK"));
    }

    #[test]
    fn broken_zip_returns_open_failed() {
        let dir = tempdir().expect("create tempdir");
        let apk = dir.path().join("broken.apk");
        std::fs::write(&apk, b"not-zip").expect("write apk");

        let envelope = parse_apk_to_envelope(&apk);
        assert!(!envelope.success);
        assert_eq!(envelope.error_code.as_deref(), Some("APK_OPEN_FAILED"));
    }

    #[test]
    fn defaults_and_channel_priority_from_manifest() {
        let manifest = r#"<manifest package="com.demo.app" android:versionCode="100" xmlns:android="http://schemas.android.com/apk/res/android"> 
            <uses-sdk android:minSdkVersion="21" android:targetSdkVersion="34" />
            <application android:label="Demo" android:icon="@mipmap/ic_launcher">
                <meta-data android:name="UMENG_CHANNEL" android:value="manifest_channel" />
            </application>
            <uses-permission android:name="android.permission.INTERNET" />
        </manifest>"#;

        let apk = build_zip_with_name(
            "app-huawei-release.apk",
            vec![
                ("AndroidManifest.xml", manifest.as_bytes()),
                ("META-INF/channel_oppo", b""),
                ("res/mipmap-xxhdpi/ic_launcher.png", b"png"),
            ],
        );

        let envelope = parse_apk_to_envelope(&apk);
        assert!(envelope.success);
        assert_eq!(envelope.data.package_name, "com.demo.app");
        assert_eq!(envelope.data.version_name, None);
        assert_eq!(envelope.data.compile_sdk_version, None);
        assert_eq!(envelope.data.channel, "manifest_channel");
        assert_eq!(envelope.data.permissions, vec!["android.permission.INTERNET"]);
        assert!(!envelope.data.icon_url.is_empty());
    }

    #[test]
    fn app_name_from_string_resource_is_resolved() {
        let manifest = r#"<manifest package="com.demo.app" xmlns:android="http://schemas.android.com/apk/res/android">
            <application android:label="@string/app_name" />
        </manifest>"#;
        let strings = r#"<resources><string name="app_name">Demo App</string></resources>"#;
        let apk = build_zip_with_name(
            "demo.apk",
            vec![
                ("AndroidManifest.xml", manifest.as_bytes()),
                ("res/values/strings.xml", strings.as_bytes()),
            ],
        );

        let envelope = parse_apk_to_envelope(&apk);
        assert!(envelope.success);
        assert_eq!(envelope.data.app_name, "Demo App");
        assert!(
            !envelope
                .warnings
                .iter()
                .any(|w| w == super::warnings::APP_NAME_UNRESOLVED)
        );
    }

    #[test]
    fn app_name_from_prefixed_string_reference_is_resolved() {
        let manifest = r#"<manifest package="com.demo.app" xmlns:android="http://schemas.android.com/apk/res/android">
            <application android:label="@com.demo.app:string/app_name" />
        </manifest>"#;
        let strings = r#"<resources><string name="app_name">Demo App From Prefix</string></resources>"#;
        let apk = build_zip_with_name(
            "demo-prefix.apk",
            vec![
                ("AndroidManifest.xml", manifest.as_bytes()),
                ("res/values/strings.xml", strings.as_bytes()),
            ],
        );

        let envelope = parse_apk_to_envelope(&apk);
        assert!(envelope.success);
        assert_eq!(envelope.data.app_name, "Demo App From Prefix");
    }

    #[test]
    fn unknown_channel_sets_warning() {
        let manifest = r#"<manifest package="com.demo.app" xmlns:android="http://schemas.android.com/apk/res/android"></manifest>"#;

        let apk = build_zip_with_name("demo-release.apk", vec![("AndroidManifest.xml", manifest.as_bytes())]);
        let envelope = parse_apk_to_envelope(&apk);

        assert!(envelope.success);
        assert_eq!(envelope.data.channel, "unknown");
        assert!(
            envelope
                .warnings
                .iter()
                .any(|w| w == super::warnings::CHANNEL_NOT_FOUND)
        );
    }

    #[test]
    fn webp_icon_can_be_extracted() {
        let manifest = r#"<manifest package="com.demo.app" xmlns:android="http://schemas.android.com/apk/res/android">
            <application android:icon="@mipmap/ic_launcher" />
        </manifest>"#;

        let apk = build_zip_with_name(
            "icon-webp.apk",
            vec![
                ("AndroidManifest.xml", manifest.as_bytes()),
                ("res/mipmap-xxhdpi/ic_launcher.webp", b"webp"),
            ],
        );

        let envelope = parse_apk_to_envelope(&apk);
        assert!(envelope.success);
        assert!(envelope.data.icon_url.ends_with(".webp"));
    }

    #[test]
    fn envelope_json_contract_keys_exist() {
        let manifest = r#"<manifest package="com.demo.app" xmlns:android="http://schemas.android.com/apk/res/android"></manifest>"#;
        let apk = build_zip_with_name("demo.apk", vec![("AndroidManifest.xml", manifest.as_bytes())]);
        let envelope = parse_apk_to_envelope(&apk);

        let json = serde_json::to_value(&envelope).expect("serialize envelope");
        assert!(json.get("success").is_some());
        assert!(json.get("data").is_some());
        assert!(json.get("errorCode").is_some());
        assert!(json.get("errorMessage").is_some());
        assert!(json.get("warnings").is_some());

        let data = json.get("data").expect("data object");
        for key in [
            "packageName",
            "appName",
            "iconUrl",
            "minSdkVersion",
            "targetSdkVersion",
            "versionCode",
            "versionName",
            "permissions",
            "signers",
            "abis",
            "channel",
        ] {
            assert!(data.get(key).is_some(), "missing key: {key}");
        }
    }

    #[test]
    fn error_message_is_sanitized() {
        let path = PathBuf::from(r"D:\sensitive\dir\sample.apk");
        let msg = format!("failed to parse {}", path.display());
        let sanitized = sanitize_error_message(msg, &path);
        assert!(!sanitized.contains(r"D:\sensitive\dir"));
        assert!(sanitized.contains("<redacted>/sample.apk"));
    }

    #[test]
    fn tauri_and_direct_parser_are_consistent() {
        let manifest = r#"<manifest package="com.demo.app" xmlns:android="http://schemas.android.com/apk/res/android"></manifest>"#;
        let apk = build_zip_with_name("demo.apk", vec![("AndroidManifest.xml", manifest.as_bytes())]);

        let direct = parse_apk_to_envelope(&apk);
        let tauri = parse_apk_tauri(apk.to_string_lossy().to_string());

        assert_eq!(direct.success, tauri.success);
        assert_eq!(direct.data.package_name, tauri.data.package_name);
        assert_eq!(direct.data.channel, tauri.data.channel);
        assert_eq!(direct.data.permissions, tauri.data.permissions);
        assert_eq!(direct.data.signers.len(), tauri.data.signers.len());
        assert_eq!(direct.data.abis, tauri.data.abis);
    }

    #[test]
    fn signer_dates_are_not_fabricated() {
        let manifest = r#"<manifest package="com.demo.app" xmlns:android="http://schemas.android.com/apk/res/android"></manifest>"#;
        let apk = build_zip_with_name(
            "signed.apk",
            vec![
                ("AndroidManifest.xml", manifest.as_bytes()),
                ("META-INF/CERT.RSA", b"dummy-cert"),
            ],
        );

        let envelope = parse_apk_to_envelope(&apk);
        assert!(envelope.success);
        if let Some(first) = envelope.data.signers.first() {
            assert!(first.valid_from.is_empty());
            assert!(first.valid_to.is_empty());
        }
    }

    fn build_zip_with_name(name: &str, files: Vec<(&str, &[u8])>) -> std::path::PathBuf {
        let dir = tempdir().expect("create tmpdir");
        let root = dir.into_path();
        let target = root.join(name);

        {
            let file = std::fs::File::create(&target).expect("create apk");
            let mut writer = zip::ZipWriter::new(file);
            let options = SimpleFileOptions::default();
            for (entry, body) in files {
                writer.start_file(entry, options).expect("start file");
                writer.write_all(body).expect("write body");
            }
            writer.finish().expect("finish zip");
        }

        target
    }
}
