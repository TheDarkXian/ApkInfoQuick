
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
    pub const ICON_MANIFEST_REF_UNRESOLVED: &str = "ICON_MANIFEST_REF_UNRESOLVED";
    pub const ICON_RESOURCE_ID_UNRESOLVED: &str = "ICON_RESOURCE_ID_UNRESOLVED";
    pub const ICON_ADAPTIVE_XML_UNRESOLVED: &str = "ICON_ADAPTIVE_XML_UNRESOLVED";
    pub const ICON_CANDIDATES_EMPTY: &str = "ICON_CANDIDATES_EMPTY";
    pub const APP_NAME_UNRESOLVED: &str = "APP_NAME_UNRESOLVED";
    pub const APP_NAME_PICKED_STRING_REF: &str = "APP_NAME_PICKED_STRING_REF";
    pub const APP_NAME_PICKED_RESOURCE_ID: &str = "APP_NAME_PICKED_RESOURCE_ID";
    pub const SIGNATURE_PARTIAL: &str = "SIGNATURE_PARTIAL";
    pub const SIGNATURE_BLOCK_DETECTED_UNPARSED: &str = "SIGNATURE_BLOCK_DETECTED_UNPARSED";
    pub const MANIFEST_BINARY_PARTIAL: &str = "MANIFEST_BINARY_PARTIAL";
}

fn push_warning(warnings: &mut Vec<String>, code: &str) {
    if !warnings.iter().any(|w| w == code) {
        warnings.push(code.to_string());
    }
}

fn push_warning_owned(warnings: &mut Vec<String>, code: String) {
    if !warnings.iter().any(|w| w == &code) {
        warnings.push(code);
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
    app_round_icon: Option<String>,
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
                if let Some(v) = attrs.get("roundIcon") {
                    info.app_round_icon = Some(v.clone());
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
    const RES_STRING_POOL_TYPE: u16 = 0x0001;
    const RES_TABLE_TYPE: u16 = 0x0002;
    const RES_TABLE_PACKAGE_TYPE: u16 = 0x0200;
    const RES_TABLE_TYPE_TYPE: u16 = 0x0201;

    #[derive(Debug, Clone)]
    enum AppLabelRef {
        StringKey(String),
        ResourceId(u32),
    }

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

        if let Some(label_ref) = parse_label_ref(&label) {
            let mut visited = BTreeSet::new();
            if let Some(v) = resolve_from_label_ref(archive, &label_ref, 0, &mut visited) {
                manifest.app_name = Some(v);
                super::push_warning(
                    warnings,
                    match label_ref {
                        AppLabelRef::StringKey(_) => super::warnings::APP_NAME_PICKED_STRING_REF,
                        AppLabelRef::ResourceId(_) => super::warnings::APP_NAME_PICKED_RESOURCE_ID,
                    },
                );
                return;
            }
        }

        manifest.app_name = None;
        super::push_warning(warnings, super::warnings::APP_NAME_UNRESOLVED);
    }

    fn parse_label_ref(label: &str) -> Option<AppLabelRef> {
        if let Some(hex) = label.strip_prefix("@0x") {
            if let Ok(id) = u32::from_str_radix(hex, 16) {
                return Some(AppLabelRef::ResourceId(id));
            }
        }

        if let Some(key) = label.strip_prefix("@string/") {
            return Some(AppLabelRef::StringKey(key.to_string()));
        }

        if let Some(raw) = label.strip_prefix('@') {
            if let Some((_, key)) = raw.rsplit_once(":string/") {
                return Some(AppLabelRef::StringKey(key.to_string()));
            }
            if let Some(key) = raw.strip_prefix("string/") {
                return Some(AppLabelRef::StringKey(key.to_string()));
            }
        }

        None
    }

    fn resolve_from_label_ref(
        archive: &mut ZipArchive<File>,
        label_ref: &AppLabelRef,
        depth: usize,
        visited: &mut BTreeSet<String>,
    ) -> Option<String> {
        if depth > 4 {
            return None;
        }

        match label_ref {
            AppLabelRef::StringKey(key) => {
                let marker = format!("string:{key}");
                if !visited.insert(marker.clone()) {
                    return None;
                }
                let result = read_string_resource(archive, key, depth, visited);
                visited.remove(&marker);
                result
            }
            AppLabelRef::ResourceId(resource_id) => {
                let marker = format!("resid:{resource_id:08x}");
                if !visited.insert(marker.clone()) {
                    return None;
                }
                let (resource_type, key) = resolve_resource_id_from_arsc(archive, *resource_id)?;
                if resource_type != "string" {
                    visited.remove(&marker);
                    return None;
                }
                let result = read_string_resource(archive, &key, depth + 1, visited);
                visited.remove(&marker);
                result
            }
        }
    }

    fn read_string_resource(
        archive: &mut ZipArchive<File>,
        key: &str,
        depth: usize,
        visited: &mut BTreeSet<String>,
    ) -> Option<String> {
        let mut candidates = archive_reader::list_entries(archive)
            .into_iter()
            .filter(|name| {
                if !name.starts_with("res/values") {
                    return false;
                }
                let Some(file_name) = name.rsplit('/').next() else {
                    return false;
                };
                file_name.starts_with("strings") && file_name.ends_with(".xml")
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| {
            string_file_priority(b)
                .cmp(&string_file_priority(a))
                .then_with(|| a.cmp(b))
        });

        for path in candidates {
            let mut content = String::new();
            let read_ok = {
                if let Ok(mut entry) = archive.by_name(&path) {
                    entry.read_to_string(&mut content).is_ok()
                } else {
                    false
                }
            };
            if !read_ok {
                continue;
            }
            if let Some(v) = find_string_in_xml(&content, key) {
                let normalized = v.trim();
                if normalized.is_empty() {
                    continue;
                }
                if let Some(next_ref) = parse_label_ref(normalized) {
                    if let Some(resolved) =
                        resolve_from_label_ref(archive, &next_ref, depth + 1, visited)
                    {
                        return Some(resolved);
                    }
                    continue;
                }
                return Some(normalized.to_string());
            }
        }
        None
    }

    fn string_file_priority(path: &str) -> i32 {
        let lower = path.to_ascii_lowercase();
        if lower.contains("/values-zh-rcn/") {
            return 300;
        }
        if lower.contains("/values-zh/") {
            return 220;
        }
        if lower == "res/values/strings.xml" {
            return 200;
        }
        if lower.starts_with("res/values/") {
            return 150;
        }
        100
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

    fn resolve_resource_id_from_arsc(
        archive: &mut ZipArchive<File>,
        resource_id: u32,
    ) -> Option<(String, String)> {
        let mut entry = archive.by_name("resources.arsc").ok()?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).ok()?;

        let package_id = ((resource_id >> 24) & 0xff) as u8;
        let type_id = ((resource_id >> 16) & 0xff) as u8;
        let entry_id = (resource_id & 0xffff) as u16;

        parse_arsc_resource_name(&bytes, package_id, type_id, entry_id)
    }

    fn parse_arsc_resource_name(
        bytes: &[u8],
        package_id: u8,
        target_type_id: u8,
        target_entry_id: u16,
    ) -> Option<(String, String)> {
        if bytes.len() < 12 || le_u16(bytes, 0)? != RES_TABLE_TYPE {
            return None;
        }
        let table_size = le_u32(bytes, 4)? as usize;
        if table_size > bytes.len() {
            return None;
        }

        let mut offset = le_u16(bytes, 2)? as usize;
        while offset + 8 <= table_size {
            let chunk_type = le_u16(bytes, offset)?;
            let header_size = le_u16(bytes, offset + 2)? as usize;
            let chunk_size = le_u32(bytes, offset + 4)? as usize;
            if chunk_size == 0 || offset + chunk_size > table_size || header_size > chunk_size {
                return None;
            }

            if chunk_type == RES_TABLE_PACKAGE_TYPE {
                let chunk = &bytes[offset..offset + chunk_size];
                if let Some(result) =
                    parse_package_chunk(chunk, package_id, target_type_id, target_entry_id)
                {
                    return Some(result);
                }
            }
            offset += chunk_size;
        }
        None
    }

    fn parse_package_chunk(
        chunk: &[u8],
        package_id: u8,
        target_type_id: u8,
        target_entry_id: u16,
    ) -> Option<(String, String)> {
        if chunk.len() < 288 {
            return None;
        }
        let pkg_id = le_u32(chunk, 8)? as u8;
        if pkg_id != package_id {
            return None;
        }

        let type_strings_offset = le_u32(chunk, 268)? as usize;
        let key_strings_offset = le_u32(chunk, 276)? as usize;
        let package_header_size = le_u16(chunk, 2)? as usize;
        if package_header_size > chunk.len() {
            return None;
        }

        let type_strings = parse_string_pool_at(chunk, type_strings_offset)?;
        let key_strings = parse_string_pool_at(chunk, key_strings_offset)?;

        let mut offset = package_header_size;
        while offset + 8 <= chunk.len() {
            let chunk_type = le_u16(chunk, offset)?;
            let header_size = le_u16(chunk, offset + 2)? as usize;
            let chunk_size = le_u32(chunk, offset + 4)? as usize;
            if chunk_size == 0 || offset + chunk_size > chunk.len() || header_size > chunk_size {
                return None;
            }

            if chunk_type == RES_TABLE_TYPE_TYPE {
                let type_chunk = &chunk[offset..offset + chunk_size];
                if let Some(result) = resolve_in_type_chunk(
                    type_chunk,
                    &type_strings,
                    &key_strings,
                    target_type_id,
                    target_entry_id,
                ) {
                    return Some(result);
                }
            }
            offset += chunk_size;
        }
        None
    }

    fn resolve_in_type_chunk(
        type_chunk: &[u8],
        type_strings: &[String],
        key_strings: &[String],
        target_type_id: u8,
        target_entry_id: u16,
    ) -> Option<(String, String)> {
        if type_chunk.len() < 32 {
            return None;
        }
        let type_id = *type_chunk.get(8)?;
        if type_id != target_type_id {
            return None;
        }
        let entry_count = le_u32(type_chunk, 12)? as usize;
        let entries_start = le_u32(type_chunk, 16)? as usize;
        let header_size = le_u16(type_chunk, 2)? as usize;
        if entries_start >= type_chunk.len() || header_size > type_chunk.len() {
            return None;
        }

        let target = target_entry_id as usize;
        if target >= entry_count {
            return None;
        }
        let entry_offset_pos = header_size + target * 4;
        if entry_offset_pos + 4 > type_chunk.len() {
            return None;
        }
        let entry_offset = le_u32(type_chunk, entry_offset_pos)? as usize;
        if entry_offset == 0xffff_ffff {
            return None;
        }
        let entry_base = entries_start + entry_offset;
        if entry_base + 8 > type_chunk.len() {
            return None;
        }
        let key_index = le_u32(type_chunk, entry_base + 4)? as usize;

        let type_name = type_strings.get((type_id - 1) as usize)?.clone();
        let key_name = key_strings.get(key_index)?.clone();
        Some((type_name, key_name))
    }

    fn parse_string_pool_at(bytes: &[u8], offset: usize) -> Option<Vec<String>> {
        if offset + 8 > bytes.len() || le_u16(bytes, offset)? != RES_STRING_POOL_TYPE {
            return None;
        }
        let chunk_size = le_u32(bytes, offset + 4)? as usize;
        if offset + chunk_size > bytes.len() {
            return None;
        }
        parse_string_pool_chunk(&bytes[offset..offset + chunk_size])
    }

    fn parse_string_pool_chunk(chunk: &[u8]) -> Option<Vec<String>> {
        if chunk.len() < 28 {
            return None;
        }
        let string_count = le_u32(chunk, 8)? as usize;
        let flags = le_u32(chunk, 16)?;
        let strings_start = le_u32(chunk, 20)? as usize;
        let utf8 = (flags & 0x0000_0100) != 0;
        let index_table_start = 28;
        let index_table_end = index_table_start + string_count * 4;
        if index_table_end > chunk.len() {
            return None;
        }

        let mut out = Vec::with_capacity(string_count);
        for i in 0..string_count {
            let off = le_u32(chunk, index_table_start + i * 4)? as usize;
            let start = strings_start + off;
            if start >= chunk.len() {
                return None;
            }
            let s = if utf8 {
                decode_utf8_pool_string(chunk, start)?
            } else {
                decode_utf16_pool_string(chunk, start)?
            };
            out.push(s);
        }
        Some(out)
    }

    fn decode_utf8_pool_string(chunk: &[u8], start: usize) -> Option<String> {
        let (_, off1) = decode_length8(chunk, start)?;
        let (byte_len, off2) = decode_length8(chunk, off1)?;
        let end = off2 + byte_len;
        if end > chunk.len() {
            return None;
        }
        Some(String::from_utf8_lossy(&chunk[off2..end]).to_string())
    }

    fn decode_utf16_pool_string(chunk: &[u8], start: usize) -> Option<String> {
        let (char_len, mut off) = decode_length16(chunk, start)?;
        let mut vals = Vec::with_capacity(char_len);
        for _ in 0..char_len {
            if off + 2 > chunk.len() {
                return None;
            }
            vals.push(u16::from_le_bytes([chunk[off], chunk[off + 1]]));
            off += 2;
        }
        Some(String::from_utf16_lossy(&vals))
    }

    fn decode_length8(chunk: &[u8], start: usize) -> Option<(usize, usize)> {
        let first = *chunk.get(start)?;
        if (first & 0x80) == 0 {
            return Some((first as usize, start + 1));
        }
        let second = *chunk.get(start + 1)?;
        Some(((((first & 0x7f) as usize) << 8) | second as usize, start + 2))
    }

    fn decode_length16(chunk: &[u8], start: usize) -> Option<(usize, usize)> {
        let first = le_u16(chunk, start)? as usize;
        if (first & 0x8000) == 0 {
            return Some((first, start + 2));
        }
        let second = le_u16(chunk, start + 2)? as usize;
        Some((((first & 0x7fff) << 16) | second, start + 4))
    }

    fn le_u16(bytes: &[u8], offset: usize) -> Option<u16> {
        let b0 = *bytes.get(offset)?;
        let b1 = *bytes.get(offset + 1)?;
        Some(u16::from_le_bytes([b0, b1]))
    }

    fn le_u32(bytes: &[u8], offset: usize) -> Option<u32> {
        let b0 = *bytes.get(offset)?;
        let b1 = *bytes.get(offset + 1)?;
        let b2 = *bytes.get(offset + 2)?;
        let b3 = *bytes.get(offset + 3)?;
        Some(u32::from_le_bytes([b0, b1, b2, b3]))
    }
}
mod icon_extractor {
    use super::*;
    const RES_STRING_POOL_TYPE: u16 = 0x0001;
    const RES_TABLE_TYPE: u16 = 0x0002;
    const RES_TABLE_PACKAGE_TYPE: u16 = 0x0200;
    const RES_TABLE_TYPE_TYPE: u16 = 0x0201;

    const ICON_EXTS: [&str; 3] = ["png", "webp", "9.png"];

    #[derive(Debug, Clone)]
    struct IconCandidate {
        entry_name: String,
        strategy_name: &'static str,
        score: i32,
    }

    #[derive(Debug, Clone)]
    enum IconRef {
        DirectPath(String),
        ResourceName { icon_type: String, name: String },
        ResourceId(u32),
    }

    pub fn extract_best_icon(
        apk_path: &Path,
        manifest: &ManifestInfo,
        archive: &mut ZipArchive<File>,
        warnings: &mut Vec<String>,
    ) -> String {
        let entries = archive_reader::list_entries(archive);

        let mut candidates: Vec<IconCandidate> = Vec::new();
        let mut had_manifest_ref = false;

        for (label, strategy) in [
            (&manifest.app_icon, "ManifestPath"),
            (&manifest.app_round_icon, "RoundIcon"),
        ] {
            if let Some(raw_ref) = label {
                had_manifest_ref = true;
                let icon_ref = parse_icon_ref(raw_ref);
                let before = candidates.len();
                collect_candidates_from_ref(
                    &icon_ref,
                    strategy,
                    &entries,
                    archive,
                    warnings,
                    &mut candidates,
                );
                if matches!(icon_ref, IconRef::ResourceId(_)) && candidates.len() == before {
                    super::push_warning(warnings, super::warnings::ICON_RESOURCE_ID_UNRESOLVED);
                }
            }
        }

        if had_manifest_ref && candidates.is_empty() {
            super::push_warning(warnings, super::warnings::ICON_MANIFEST_REF_UNRESOLVED);
        }

        if candidates.is_empty() {
            candidates.extend(collect_heuristic_candidates(&entries));
        }

        if candidates.is_empty() {
            super::push_warning(warnings, super::warnings::ICON_CANDIDATES_EMPTY);
            super::push_warning(warnings, super::warnings::ICON_NOT_FOUND);
            return String::new();
        }

        candidates.sort_by(|a, b| b.score.cmp(&a.score));
        let mut seen = BTreeSet::new();
        candidates.retain(|item| seen.insert(item.entry_name.clone()));

        for candidate in candidates {
            let strategy_name = candidate.strategy_name;
            if let Ok(mut entry) = archive.by_name(&candidate.entry_name) {
                let mut buf = Vec::new();
                if entry.read_to_end(&mut buf).is_ok() {
                    if let Some(url) = write_icon_to_temp(apk_path, &candidate.entry_name, &buf) {
                        super::push_warning_owned(
                            warnings,
                            strategy_tracking_code(strategy_name),
                        );
                        return url;
                    }
                }
            }
        }

        super::push_warning(warnings, super::warnings::ICON_NOT_FOUND);
        String::new()
    }

    fn collect_candidates_from_ref(
        icon_ref: &IconRef,
        strategy_name: &'static str,
        entries: &[String],
        archive: &mut ZipArchive<File>,
        warnings: &mut Vec<String>,
        out: &mut Vec<IconCandidate>,
    ) {
        match icon_ref {
            IconRef::DirectPath(path) => {
                if entries.iter().any(|entry| entry == path) {
                    out.push(IconCandidate {
                        entry_name: path.clone(),
                        strategy_name,
                        score: score_entry(path, Some(path), true) + 130,
                    });
                }
            }
            IconRef::ResourceName { icon_type, name } => {
                out.extend(find_by_resource_name(entries, icon_type, name, strategy_name));
                out.extend(find_adaptive_icon_refs(entries, archive, icon_type, name, warnings));
            }
            IconRef::ResourceId(id) => {
                if let Some((icon_type, name)) = resolve_resource_id_from_arsc(archive, *id) {
                    out.extend(find_by_resource_name(entries, &icon_type, &name, "ResourceIdArsc"));
                    out.extend(find_adaptive_icon_refs(
                        entries,
                        archive,
                        &icon_type,
                        &name,
                        warnings,
                    ));
                }
            }
        }
    }

    fn parse_icon_ref(raw: &str) -> IconRef {
        if !raw.starts_with('@') {
            return IconRef::DirectPath(raw.to_string());
        }

        if let Some(hex) = raw.strip_prefix("@0x") {
            if let Ok(value) = u32::from_str_radix(hex, 16) {
                return IconRef::ResourceId(value);
            }
        }

        let normalized = raw.trim_start_matches('@');
        let no_pkg = normalized
            .rsplit_once(':')
            .map(|(_, right)| right)
            .unwrap_or(normalized);
        if let Some((icon_type, name)) = no_pkg.split_once('/') {
            return IconRef::ResourceName {
                icon_type: icon_type.to_string(),
                name: name.to_string(),
            };
        }

        IconRef::DirectPath(raw.to_string())
    }

    fn find_by_resource_name(
        entries: &[String],
        icon_type: &str,
        icon_name: &str,
        strategy_name: &'static str,
    ) -> Vec<IconCandidate> {
        let mut out = Vec::new();
        for entry in entries {
            if !is_icon_asset(entry) {
                continue;
            }
            if !entry.starts_with("res/") {
                continue;
            }
            if !entry.contains(&format!("/{icon_name}.")) {
                continue;
            }
            if !entry.contains(&format!("/{icon_type}")) {
                continue;
            }
            out.push(IconCandidate {
                entry_name: entry.clone(),
                strategy_name,
                score: score_entry(entry, Some(icon_name), true),
            });
        }
        out
    }

    fn find_adaptive_icon_refs(
        entries: &[String],
        archive: &mut ZipArchive<File>,
        icon_type: &str,
        icon_name: &str,
        warnings: &mut Vec<String>,
    ) -> Vec<IconCandidate> {
        let xml_paths = entries
            .iter()
            .filter(|entry| {
                entry.starts_with("res/")
                    && entry.contains(&format!("/{icon_type}"))
                    && entry.contains(&format!("/{icon_name}.xml"))
            })
            .cloned()
            .collect::<Vec<_>>();

        let mut out = Vec::new();
        for xml_path in xml_paths {
            if !xml_path.contains("-v26/") && !xml_path.contains("anydpi") {
                continue;
            }
            if let Ok(mut entry) = archive.by_name(&xml_path) {
                let mut bytes = Vec::new();
                if entry.read_to_end(&mut bytes).is_err() {
                    continue;
                }
                if !bytes.starts_with(b"<") {
                    super::push_warning(warnings, super::warnings::ICON_ADAPTIVE_XML_UNRESOLVED);
                    continue;
                }
                for res_ref in parse_adaptive_references(&bytes) {
                    let ref_item = parse_icon_ref(&res_ref);
                    if let IconRef::ResourceName { icon_type, name } = ref_item {
                        out.extend(find_by_resource_name(
                            entries,
                            &icon_type,
                            &name,
                            "AdaptiveXml",
                        ));
                    }
                }
            }
        }

        if out.is_empty() && entries.iter().any(|e| e.contains("anydpi-v26") && e.ends_with(".xml")) {
            super::push_warning(warnings, super::warnings::ICON_ADAPTIVE_XML_UNRESOLVED);
        }

        out
    }

    fn parse_adaptive_references(xml: &[u8]) -> Vec<String> {
        let mut reader = Reader::from_reader(xml);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();
        let mut out = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_lowercase();
                    if tag.ends_with("foreground") || tag.ends_with("background") {
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_lowercase();
                            if key.ends_with("drawable") {
                                if let Ok(v) = attr.decode_and_unescape_value(reader.decoder()) {
                                    let value = v.to_string();
                                    if value.starts_with('@') {
                                        out.push(value);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        out
    }

    fn collect_heuristic_candidates(entries: &[String]) -> Vec<IconCandidate> {
        entries
            .iter()
            .filter(|entry| is_icon_asset(entry))
            .map(|entry| IconCandidate {
                entry_name: entry.clone(),
                strategy_name: "HeuristicFallback",
                score: score_entry(entry, None, false),
            })
            .collect()
    }

    fn is_icon_asset(entry: &str) -> bool {
        if !entry.starts_with("res/") {
            return false;
        }
        if !(entry.starts_with("res/mipmap") || entry.starts_with("res/drawable")) {
            return false;
        }
        if is_blacklisted(entry) {
            return false;
        }
        ICON_EXTS.iter().any(|ext| entry.ends_with(&format!(".{ext}")))
    }

    fn is_blacklisted(entry: &str) -> bool {
        let lower = entry.to_lowercase();
        [
            "notification",
            "notify",
            "splash",
            "banner",
            "status_bar",
            "stat_sys",
            "push",
            "ad_",
            "promo",
        ]
        .iter()
        .any(|needle| lower.contains(needle))
    }

    fn score_entry(entry: &str, expected_name: Option<&str>, from_manifest: bool) -> i32 {
        let lower = entry.to_lowercase();
        let mut score = if from_manifest { 100 } else { 40 };
        score += density_score(&lower);

        if lower.contains("/mipmap") {
            score += 30;
        }
        if lower.contains("/drawable") {
            score += 12;
        }
        if lower.ends_with(".png") {
            score += 6;
        }
        if lower.ends_with(".webp") {
            score += 4;
        }
        if lower.ends_with(".9.png") {
            score -= 10;
        }
        if lower.contains("launcher") {
            score += 8;
        }

        if let Some(name) = expected_name {
            if lower.contains(&format!("/{name}.")) {
                score += 20;
            }
        }

        score
    }

    fn density_score(path: &str) -> i32 {
        let table = [
            ("xxxhdpi", 60),
            ("xxhdpi", 54),
            ("xhdpi", 48),
            ("hdpi", 42),
            ("mdpi", 36),
            ("ldpi", 24),
            ("anydpi", 20),
            ("tvdpi", 18),
            ("nodpi", 12),
        ];
        for (needle, score) in table {
            if path.contains(needle) {
                return score;
            }
        }
        8
    }

    fn resolve_resource_id_from_arsc(
        archive: &mut ZipArchive<File>,
        resource_id: u32,
    ) -> Option<(String, String)> {
        let mut entry = archive.by_name("resources.arsc").ok()?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).ok()?;

        let package_id = ((resource_id >> 24) & 0xff) as u8;
        let type_id = ((resource_id >> 16) & 0xff) as u8;
        let entry_id = (resource_id & 0xffff) as u16;

        parse_arsc_resource_name(&bytes, package_id, type_id, entry_id)
    }

    fn parse_arsc_resource_name(
        bytes: &[u8],
        package_id: u8,
        target_type_id: u8,
        target_entry_id: u16,
    ) -> Option<(String, String)> {
        if bytes.len() < 12 {
            return None;
        }
        if le_u16(bytes, 0)? != RES_TABLE_TYPE {
            return None;
        }
        let table_size = le_u32(bytes, 4)? as usize;
        if table_size > bytes.len() {
            return None;
        }

        let mut offset = le_u16(bytes, 2)? as usize;
        while offset + 8 <= table_size {
            let chunk_type = le_u16(bytes, offset)?;
            let header_size = le_u16(bytes, offset + 2)? as usize;
            let chunk_size = le_u32(bytes, offset + 4)? as usize;
            if chunk_size == 0 || offset + chunk_size > table_size || header_size > chunk_size {
                return None;
            }

            if chunk_type == RES_TABLE_PACKAGE_TYPE {
                let chunk = &bytes[offset..offset + chunk_size];
                if let Some(result) =
                    parse_package_chunk(chunk, package_id, target_type_id, target_entry_id)
                {
                    return Some(result);
                }
            }
            offset += chunk_size;
        }
        None
    }

    fn parse_package_chunk(
        chunk: &[u8],
        package_id: u8,
        target_type_id: u8,
        target_entry_id: u16,
    ) -> Option<(String, String)> {
        if chunk.len() < 288 {
            return None;
        }
        let pkg_id = le_u32(chunk, 8)? as u8;
        if pkg_id != package_id {
            return None;
        }

        let type_strings_offset = le_u32(chunk, 268)? as usize;
        let key_strings_offset = le_u32(chunk, 276)? as usize;
        let package_header_size = le_u16(chunk, 2)? as usize;
        if package_header_size > chunk.len() {
            return None;
        }

        let type_strings = parse_string_pool_at(chunk, type_strings_offset)?;
        let key_strings = parse_string_pool_at(chunk, key_strings_offset)?;

        let mut offset = package_header_size;
        while offset + 8 <= chunk.len() {
            let chunk_type = le_u16(chunk, offset)?;
            let header_size = le_u16(chunk, offset + 2)? as usize;
            let chunk_size = le_u32(chunk, offset + 4)? as usize;
            if chunk_size == 0 || offset + chunk_size > chunk.len() || header_size > chunk_size {
                return None;
            }

            if chunk_type == RES_TABLE_TYPE_TYPE {
                let type_chunk = &chunk[offset..offset + chunk_size];
                if let Some(result) = resolve_in_type_chunk(
                    type_chunk,
                    &type_strings,
                    &key_strings,
                    target_type_id,
                    target_entry_id,
                ) {
                    return Some(result);
                }
            }
            offset += chunk_size;
        }
        None
    }

    fn resolve_in_type_chunk(
        type_chunk: &[u8],
        type_strings: &[String],
        key_strings: &[String],
        target_type_id: u8,
        target_entry_id: u16,
    ) -> Option<(String, String)> {
        if type_chunk.len() < 32 {
            return None;
        }
        let type_id = *type_chunk.get(8)?;
        if type_id != target_type_id {
            return None;
        }
        let entry_count = le_u32(type_chunk, 12)? as usize;
        let entries_start = le_u32(type_chunk, 16)? as usize;
        let header_size = le_u16(type_chunk, 2)? as usize;
        if entries_start >= type_chunk.len() || header_size > type_chunk.len() {
            return None;
        }

        let target = target_entry_id as usize;
        if target >= entry_count {
            return None;
        }

        let entry_offset_pos = header_size + target * 4;
        if entry_offset_pos + 4 > type_chunk.len() {
            return None;
        }
        let entry_offset = le_u32(type_chunk, entry_offset_pos)? as usize;
        if entry_offset == 0xffff_ffff {
            return None;
        }

        let entry_base = entries_start + entry_offset;
        if entry_base + 8 > type_chunk.len() {
            return None;
        }
        let key_index = le_u32(type_chunk, entry_base + 4)? as usize;

        let type_name = type_strings.get((type_id - 1) as usize)?.clone();
        let key_name = key_strings.get(key_index)?.clone();
        Some((type_name, key_name))
    }

    fn parse_string_pool_at(bytes: &[u8], offset: usize) -> Option<Vec<String>> {
        if offset + 8 > bytes.len() {
            return None;
        }
        if le_u16(bytes, offset)? != RES_STRING_POOL_TYPE {
            return None;
        }
        let chunk_size = le_u32(bytes, offset + 4)? as usize;
        if offset + chunk_size > bytes.len() {
            return None;
        }
        parse_string_pool_chunk(&bytes[offset..offset + chunk_size])
    }

    fn parse_string_pool_chunk(chunk: &[u8]) -> Option<Vec<String>> {
        if chunk.len() < 28 {
            return None;
        }

        let string_count = le_u32(chunk, 8)? as usize;
        let flags = le_u32(chunk, 16)?;
        let strings_start = le_u32(chunk, 20)? as usize;
        let utf8 = (flags & 0x0000_0100) != 0;

        let index_table_start = 28;
        let index_table_end = index_table_start + string_count * 4;
        if index_table_end > chunk.len() {
            return None;
        }

        let mut out = Vec::with_capacity(string_count);
        for i in 0..string_count {
            let off = le_u32(chunk, index_table_start + i * 4)? as usize;
            let start = strings_start + off;
            if start >= chunk.len() {
                return None;
            }
            let s = if utf8 {
                decode_utf8_pool_string(chunk, start)?
            } else {
                decode_utf16_pool_string(chunk, start)?
            };
            out.push(s);
        }
        Some(out)
    }

    fn decode_utf8_pool_string(chunk: &[u8], start: usize) -> Option<String> {
        let (_, off1) = decode_length8(chunk, start)?;
        let (byte_len, off2) = decode_length8(chunk, off1)?;
        let end = off2 + byte_len;
        if end > chunk.len() {
            return None;
        }
        Some(String::from_utf8_lossy(&chunk[off2..end]).to_string())
    }

    fn decode_utf16_pool_string(chunk: &[u8], start: usize) -> Option<String> {
        let (char_len, mut off) = decode_length16(chunk, start)?;
        let mut vals = Vec::with_capacity(char_len);
        for _ in 0..char_len {
            if off + 2 > chunk.len() {
                return None;
            }
            vals.push(u16::from_le_bytes([chunk[off], chunk[off + 1]]));
            off += 2;
        }
        Some(String::from_utf16_lossy(&vals))
    }

    fn decode_length8(chunk: &[u8], start: usize) -> Option<(usize, usize)> {
        let first = *chunk.get(start)?;
        if (first & 0x80) == 0 {
            return Some((first as usize, start + 1));
        }
        let second = *chunk.get(start + 1)?;
        Some(((((first & 0x7f) as usize) << 8) | second as usize, start + 2))
    }

    fn decode_length16(chunk: &[u8], start: usize) -> Option<(usize, usize)> {
        let first = le_u16(chunk, start)? as usize;
        if (first & 0x8000) == 0 {
            return Some((first, start + 2));
        }
        let second = le_u16(chunk, start + 2)? as usize;
        Some((((first & 0x7fff) << 16) | second, start + 4))
    }

    fn le_u16(bytes: &[u8], offset: usize) -> Option<u16> {
        let b0 = *bytes.get(offset)?;
        let b1 = *bytes.get(offset + 1)?;
        Some(u16::from_le_bytes([b0, b1]))
    }

    fn le_u32(bytes: &[u8], offset: usize) -> Option<u32> {
        let b0 = *bytes.get(offset)?;
        let b1 = *bytes.get(offset + 1)?;
        let b2 = *bytes.get(offset + 2)?;
        let b3 = *bytes.get(offset + 3)?;
        Some(u32::from_le_bytes([b0, b1, b2, b3]))
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

    fn strategy_tracking_code(strategy_name: &str) -> String {
        let mut out = String::from("ICON_PICKED_");
        let mut prev_underscore = false;
        for ch in strategy_name.chars() {
            if ch.is_ascii_alphanumeric() {
                if ch.is_ascii_uppercase() && !out.ends_with('_') && !prev_underscore {
                    out.push('_');
                }
                out.push(ch.to_ascii_uppercase());
                prev_underscore = false;
            } else if !prev_underscore {
                out.push('_');
                prev_underscore = true;
            }
        }
        while out.ends_with('_') {
            out.pop();
        }
        out
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
    fn app_name_from_resource_id_reference_is_resolved() {
        let manifest = r#"<manifest package="com.demo.app" xmlns:android="http://schemas.android.com/apk/res/android">
            <application android:label="@0x7f010000" />
        </manifest>"#;
        let strings = r#"<resources><string name="app_name">Demo App Resource Id</string></resources>"#;
        let arsc = build_minimal_arsc("string", 1, "app_name");
        let apk = build_zip_with_name(
            "demo-resource-id.apk",
            vec![
                ("AndroidManifest.xml", manifest.as_bytes()),
                ("resources.arsc", &arsc),
                ("res/values/strings.xml", strings.as_bytes()),
            ],
        );

        let envelope = parse_apk_to_envelope(&apk);
        assert!(envelope.success);
        assert_eq!(envelope.data.app_name, "Demo App Resource Id");
    }

    #[test]
    fn app_name_prefers_zh_rcn_over_zh_and_default() {
        let manifest = r#"<manifest package="com.demo.app" xmlns:android="http://schemas.android.com/apk/res/android">
            <application android:label="@string/app_name" />
        </manifest>"#;
        let default_strings = r#"<resources><string name="app_name">Default App</string></resources>"#;
        let zh_strings = r#"<resources><string name="app_name">中文应用</string></resources>"#;
        let zh_rcn_strings = r#"<resources><string name="app_name">中文应用（中国）</string></resources>"#;
        let apk = build_zip_with_name(
            "demo-locale.apk",
            vec![
                ("AndroidManifest.xml", manifest.as_bytes()),
                ("res/values/strings.xml", default_strings.as_bytes()),
                ("res/values-zh/strings.xml", zh_strings.as_bytes()),
                ("res/values-zh-rCN/strings.xml", zh_rcn_strings.as_bytes()),
            ],
        );

        let envelope = parse_apk_to_envelope(&apk);
        assert!(envelope.success);
        assert_eq!(envelope.data.app_name, "中文应用（中国）");
    }

    #[test]
    fn app_name_indirect_string_reference_is_resolved() {
        let manifest = r#"<manifest package="com.demo.app" xmlns:android="http://schemas.android.com/apk/res/android">
            <application android:label="@string/app_name" />
        </manifest>"#;
        let strings = r#"<resources>
            <string name="app_name">@string/app_name_real</string>
            <string name="app_name_real">Indirect Name</string>
        </resources>"#;
        let apk = build_zip_with_name(
            "demo-indirect.apk",
            vec![
                ("AndroidManifest.xml", manifest.as_bytes()),
                ("res/values/strings.xml", strings.as_bytes()),
            ],
        );

        let envelope = parse_apk_to_envelope(&apk);
        assert!(envelope.success);
        assert_eq!(envelope.data.app_name, "Indirect Name");
        assert!(
            envelope
                .warnings
                .iter()
                .any(|w| w == super::warnings::APP_NAME_PICKED_STRING_REF)
        );
    }

    #[test]
    fn app_name_indirect_cycle_does_not_loop_and_falls_back() {
        let manifest = r#"<manifest package="com.demo.app" xmlns:android="http://schemas.android.com/apk/res/android">
            <application android:label="@string/a" />
        </manifest>"#;
        let strings = r#"<resources>
            <string name="a">@string/b</string>
            <string name="b">@string/a</string>
        </resources>"#;
        let apk = build_zip_with_name(
            "demo-cycle.apk",
            vec![
                ("AndroidManifest.xml", manifest.as_bytes()),
                ("res/values/strings.xml", strings.as_bytes()),
            ],
        );

        let envelope = parse_apk_to_envelope(&apk);
        assert!(envelope.success);
        assert!(
            envelope
                .warnings
                .iter()
                .any(|w| w == super::warnings::APP_NAME_UNRESOLVED)
        );
    }

    #[test]
    fn app_name_blank_value_is_skipped_and_fallback_locale_is_used() {
        let manifest = r#"<manifest package="com.demo.app" xmlns:android="http://schemas.android.com/apk/res/android">
            <application android:label="@string/app_name" />
        </manifest>"#;
        let zh_rcn_strings = r#"<resources><string name="app_name">   </string></resources>"#;
        let zh_strings = r#"<resources><string name="app_name">中文名称</string></resources>"#;
        let apk = build_zip_with_name(
            "demo-blank-locale.apk",
            vec![
                ("AndroidManifest.xml", manifest.as_bytes()),
                ("res/values-zh-rCN/strings.xml", zh_rcn_strings.as_bytes()),
                ("res/values-zh/strings.xml", zh_strings.as_bytes()),
            ],
        );

        let envelope = parse_apk_to_envelope(&apk);
        assert!(envelope.success);
        assert_eq!(envelope.data.app_name, "中文名称");
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
        assert!(
            envelope
                .warnings
                .iter()
                .any(|item| item == "ICON_PICKED_MANIFEST_PATH")
        );
    }

    #[test]
    fn round_icon_is_used_as_fallback_source() {
        let manifest = r#"<manifest package="com.demo.app" xmlns:android="http://schemas.android.com/apk/res/android">
            <application android:roundIcon="@mipmap/ic_launcher_round" />
        </manifest>"#;

        let apk = build_zip_with_name(
            "icon-round.apk",
            vec![
                ("AndroidManifest.xml", manifest.as_bytes()),
                ("res/mipmap-xxhdpi/ic_launcher_round.png", b"png"),
            ],
        );

        let envelope = parse_apk_to_envelope(&apk);
        assert!(envelope.success);
        assert!(envelope.data.icon_url.contains("png"));
        assert!(
            envelope
                .warnings
                .iter()
                .any(|item| item == "ICON_PICKED_ROUND_ICON")
        );
    }

    #[test]
    fn package_prefixed_drawable_reference_can_be_extracted() {
        let manifest = r#"<manifest package="com.demo.app" xmlns:android="http://schemas.android.com/apk/res/android">
            <application android:icon="@com.demo.app:drawable/app_logo" />
        </manifest>"#;

        let apk = build_zip_with_name(
            "icon-pkg-drawable.apk",
            vec![
                ("AndroidManifest.xml", manifest.as_bytes()),
                ("res/drawable-xxhdpi/app_logo.png", b"png"),
            ],
        );

        let envelope = parse_apk_to_envelope(&apk);
        assert!(envelope.success);
        assert!(envelope.data.icon_url.ends_with(".png"));
        assert!(
            envelope
                .warnings
                .iter()
                .any(|item| item == "ICON_PICKED_MANIFEST_PATH")
        );
    }

    #[test]
    fn adaptive_icon_xml_foreground_reference_can_be_extracted() {
        let manifest = r#"<manifest package="com.demo.app" xmlns:android="http://schemas.android.com/apk/res/android">
            <application android:icon="@mipmap/ic_launcher" />
        </manifest>"#;
        let adaptive = r#"<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
            <background android:drawable="@color/ic_bg" />
            <foreground android:drawable="@drawable/ic_fg" />
        </adaptive-icon>"#;

        let apk = build_zip_with_name(
            "icon-adaptive.apk",
            vec![
                ("AndroidManifest.xml", manifest.as_bytes()),
                ("res/mipmap-anydpi-v26/ic_launcher.xml", adaptive.as_bytes()),
                ("res/drawable-xxhdpi/ic_fg.png", b"png"),
            ],
        );

        let envelope = parse_apk_to_envelope(&apk);
        assert!(envelope.success);
        assert!(envelope.data.icon_url.ends_with(".png"));
    }

    #[test]
    fn resource_id_icon_reference_can_be_resolved_from_arsc() {
        let manifest = r#"<manifest package="com.demo.app" xmlns:android="http://schemas.android.com/apk/res/android">
            <application android:icon="@0x7f020000" />
        </manifest>"#;
        let arsc = build_minimal_arsc_drawable("ic_launcher");

        let apk = build_zip_with_name(
            "icon-resource-id.apk",
            vec![
                ("AndroidManifest.xml", manifest.as_bytes()),
                ("resources.arsc", &arsc),
                ("res/drawable-xxhdpi/ic_launcher.png", b"png"),
            ],
        );

        let envelope = parse_apk_to_envelope(&apk);
        assert!(envelope.success);
        assert!(envelope.data.icon_url.ends_with(".png"));
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

    fn build_minimal_arsc_drawable(name: &str) -> Vec<u8> {
        build_minimal_arsc("drawable", 2, name)
    }

    fn build_minimal_arsc(resource_type: &str, type_id: u8, name: &str) -> Vec<u8> {
        fn push_u16(out: &mut Vec<u8>, value: u16) {
            out.extend_from_slice(&value.to_le_bytes());
        }
        fn push_u32(out: &mut Vec<u8>, value: u32) {
            out.extend_from_slice(&value.to_le_bytes());
        }
        fn build_string_pool(strings: &[&str]) -> Vec<u8> {
            let mut out = Vec::new();
            let mut data = Vec::new();
            let mut offsets = Vec::new();
            for s in strings {
                offsets.push(data.len() as u32);
                let bytes = s.as_bytes();
                data.push(bytes.len() as u8);
                data.push(bytes.len() as u8);
                data.extend_from_slice(bytes);
                data.push(0);
            }
            while data.len() % 4 != 0 {
                data.push(0);
            }

            let header_size = 28u16;
            let chunk_size = header_size as usize + offsets.len() * 4 + data.len();
            push_u16(&mut out, 0x0001);
            push_u16(&mut out, header_size);
            push_u32(&mut out, chunk_size as u32);
            push_u32(&mut out, strings.len() as u32);
            push_u32(&mut out, 0);
            push_u32(&mut out, 0x0000_0100);
            push_u32(&mut out, (header_size as usize + offsets.len() * 4) as u32);
            push_u32(&mut out, 0);
            for off in offsets {
                push_u32(&mut out, off);
            }
            out.extend_from_slice(&data);
            out
        }

        let type_strings = build_string_pool(&[resource_type]);
        let key_strings = build_string_pool(&[name]);

        let mut type_chunk = Vec::new();
        let type_header_size = 84u16;
        let entry_count = 1u32;
        let entries_start = type_header_size as u32 + entry_count * 4;
        let entry_size = 8u16;
        let value_size = 8u16;
        let chunk_size = entries_start as usize + entry_size as usize + value_size as usize;

        push_u16(&mut type_chunk, 0x0201);
        push_u16(&mut type_chunk, type_header_size);
        push_u32(&mut type_chunk, chunk_size as u32);
        type_chunk.push(type_id);
        type_chunk.push(0);
        push_u16(&mut type_chunk, 0);
        push_u32(&mut type_chunk, entry_count);
        push_u32(&mut type_chunk, entries_start);
        push_u32(&mut type_chunk, 64); // ResTable_config size
        type_chunk.resize(type_header_size as usize, 0);
        push_u32(&mut type_chunk, 0); // first entry offset
        push_u16(&mut type_chunk, entry_size);
        push_u16(&mut type_chunk, 0); // flags
        push_u32(&mut type_chunk, 0); // key index
        push_u16(&mut type_chunk, value_size);
        type_chunk.push(0);
        type_chunk.push(0x03); // TYPE_STRING
        push_u32(&mut type_chunk, 0);

        let package_header_size = 288u16;
        let type_strings_offset = package_header_size as u32;
        let key_strings_offset = type_strings_offset + type_strings.len() as u32;
        let package_size =
            package_header_size as usize + type_strings.len() + key_strings.len() + type_chunk.len();

        let mut package_chunk = Vec::new();
        push_u16(&mut package_chunk, 0x0200);
        push_u16(&mut package_chunk, package_header_size);
        push_u32(&mut package_chunk, package_size as u32);
        push_u32(&mut package_chunk, 0x7f); // package id
        package_chunk.resize(8 + 4 + 256, 0); // package name utf16[128]
        push_u32(&mut package_chunk, type_strings_offset);
        push_u32(&mut package_chunk, 0);
        push_u32(&mut package_chunk, key_strings_offset);
        push_u32(&mut package_chunk, 0);
        push_u32(&mut package_chunk, 0);
        package_chunk.resize(package_header_size as usize, 0);
        package_chunk.extend_from_slice(&type_strings);
        package_chunk.extend_from_slice(&key_strings);
        package_chunk.extend_from_slice(&type_chunk);

        let table_header_size = 12u16;
        let table_size = table_header_size as usize + package_chunk.len();
        let mut table = Vec::new();
        push_u16(&mut table, 0x0002);
        push_u16(&mut table, table_header_size);
        push_u32(&mut table, table_size as u32);
        push_u32(&mut table, 1); // package count
        table.extend_from_slice(&package_chunk);
        table
    }
}
