import { describe, it, expect } from "vitest";
import { quoteCliArg, normalizeDirPath, pathsEqual } from "../path-utils";

// ── quoteCliArg ──

describe("quoteCliArg", () => {
  it("wraps plain paths in double quotes", () => {
    expect(quoteCliArg("/home/dallas/code")).toBe('"/home/dallas/code"');
  });

  it("escapes backslashes", () => {
    expect(quoteCliArg("C:\\Users\\dallas")).toBe('"C:\\\\Users\\\\dallas"');
  });

  it("escapes double quotes", () => {
    expect(quoteCliArg('say "hello"')).toBe('"say \\"hello\\""');
  });

  it("returns null for paths containing newline", () => {
    expect(quoteCliArg("path\ninjection")).toBeNull();
    expect(quoteCliArg("path\rinjection")).toBeNull();
  });

  it("handles paths with spaces", () => {
    expect(quoteCliArg("/home/dallas/my project")).toBe('"/home/dallas/my project"');
  });

  it("handles empty string", () => {
    expect(quoteCliArg("")).toBe('""');
  });
});

// ── normalizeDirPath ──

describe("normalizeDirPath", () => {
  it("removes trailing slash from unix paths", () => {
    expect(normalizeDirPath("/home/dallas/code/")).toBe("/home/dallas/code");
  });

  it("removes trailing backslash from windows paths", () => {
    expect(normalizeDirPath("C:\\Users\\dallas\\")).toBe("C:\\Users\\dallas");
  });

  it("preserves unix root /", () => {
    expect(normalizeDirPath("/")).toBe("/");
  });

  it("preserves windows root C:\\ (backslash)", () => {
    expect(normalizeDirPath("C:\\")).toBe("C:\\");
  });

  it("preserves windows root C:/ (forward slash)", () => {
    expect(normalizeDirPath("C:/")).toBe("C:/");
  });

  it("does not touch paths without trailing separator", () => {
    expect(normalizeDirPath("/home/dallas")).toBe("/home/dallas");
    expect(normalizeDirPath("C:\\Users")).toBe("C:\\Users");
  });

  it("handles empty string", () => {
    expect(normalizeDirPath("")).toBe("");
  });
});

// ── pathsEqual ──

describe("pathsEqual", () => {
  it("compares unix paths case-sensitively", () => {
    expect(pathsEqual("/home/dallas", "/home/dallas")).toBe(true);
    expect(pathsEqual("/home/dallas", "/home/Dallas")).toBe(false);
  });

  it("compares windows paths case-insensitively", () => {
    expect(pathsEqual("C:\\Users\\Dallas", "c:\\users\\dallas")).toBe(true);
    expect(pathsEqual("D:/Projects", "d:/projects")).toBe(true);
  });

  it("returns false for different paths", () => {
    expect(pathsEqual("/home/dallas", "/home/other")).toBe(false);
    expect(pathsEqual("C:\\Users\\A", "C:\\Users\\B")).toBe(false);
  });

  it("treats one windows and one unix path as case-insensitive", () => {
    // If either path has a drive letter, comparison is case-insensitive
    expect(pathsEqual("C:/foo", "c:/foo")).toBe(true);
  });
});
