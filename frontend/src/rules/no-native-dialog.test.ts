import { describe, expect, it } from "vitest";

const BANNED_PATTERN = /\b(alert|confirm|prompt)\s*\(/g;

describe("no native browser dialogs", () => {
  it("does not use alert/confirm/prompt in frontend source", () => {
    const matched: string[] = [];
    const modules = import.meta.glob("../**/*.{ts,tsx,js,jsx}", {
      query: "?raw",
      import: "default",
      eager: true
    }) as Record<string, string>;

    Object.entries(modules).forEach(([file, content]) => {
      if (BANNED_PATTERN.test(content)) {
        matched.push(file);
      }
      BANNED_PATTERN.lastIndex = 0;
    });

    expect(matched).toEqual([]);
  });
});
