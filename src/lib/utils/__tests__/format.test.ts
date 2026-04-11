import { describe, it, expect } from "vitest";
import {
  formatCost,
  formatTokenCount,
  formatDuration,
  formatCostDisplay,
  truncate,
  snippetAround,
  formatBytes,
  formatPasteSize,
  splitPath,
  fileName,
  isAbsolutePath,
  cwdDisplayLabel,
  formatInstallCount,
} from "../format";

// ── formatCost ──

describe("formatCost", () => {
  it("shows 2 decimals for values >= $1", () => {
    expect(formatCost(1)).toBe("$1.00");
    expect(formatCost(3.5)).toBe("$3.50");
    expect(formatCost(100)).toBe("$100.00");
  });

  it("shows 4 decimals for values >= $0.01 and < $1", () => {
    expect(formatCost(0.01)).toBe("$0.0100");
    expect(formatCost(0.5)).toBe("$0.5000");
    expect(formatCost(0.9999)).toBe("$0.9999");
  });

  it("shows 6 decimals for values < $0.01", () => {
    expect(formatCost(0.001)).toBe("$0.001000");
    expect(formatCost(0.000001)).toBe("$0.000001");
  });
});

// ── formatCostDisplay ──

describe("formatCostDisplay", () => {
  it("shows $0.00 for zero", () => {
    expect(formatCostDisplay(0)).toBe("$0.00");
  });

  it("shows <$0.01 for tiny amounts", () => {
    expect(formatCostDisplay(0.001)).toBe("<$0.01");
    expect(formatCostDisplay(0.00999)).toBe("<$0.01");
  });

  it("shows 2 decimals for amounts >= $0.01", () => {
    expect(formatCostDisplay(0.01)).toBe("$0.01");
    expect(formatCostDisplay(1.5)).toBe("$1.50");
  });
});

// ── formatTokenCount ──

describe("formatTokenCount", () => {
  it("shows raw count below 1k", () => {
    expect(formatTokenCount(0)).toBe("0");
    expect(formatTokenCount(999)).toBe("999");
  });

  it("shows k suffix for 1k-999k", () => {
    expect(formatTokenCount(1000)).toBe("1.0k");
    expect(formatTokenCount(1500)).toBe("1.5k");
    expect(formatTokenCount(999_999)).toBe("1000.0k");
  });

  it("shows m suffix for >= 1M", () => {
    expect(formatTokenCount(1_000_000)).toBe("1.0m");
    expect(formatTokenCount(2_500_000)).toBe("2.5m");
  });
});

// ── formatDuration ──

describe("formatDuration", () => {
  it("returns empty string for zero or negative", () => {
    expect(formatDuration(0)).toBe("");
    expect(formatDuration(-1)).toBe("");
  });

  it("shows ms for sub-second durations", () => {
    expect(formatDuration(500)).toBe("500ms");
    expect(formatDuration(999)).toBe("999ms");
  });

  it("shows seconds for 1s-59s", () => {
    expect(formatDuration(1000)).toBe("1.0s");
    expect(formatDuration(5500)).toBe("5.5s");
    expect(formatDuration(59_999)).toBe("60.0s");
  });

  it("shows m and s for minute-range durations", () => {
    expect(formatDuration(60_000)).toBe("1m 0s");
    expect(formatDuration(90_000)).toBe("1m 30s");
    expect(formatDuration(3599_000)).toBe("59m 59s");
  });

  it("shows h, m, s for hour-range durations", () => {
    expect(formatDuration(3_600_000)).toBe("1h 0m 0s");
    expect(formatDuration(3_661_000)).toBe("1h 1m 1s");
  });
});

// ── truncate ──

describe("truncate", () => {
  it("returns string unchanged when within limit", () => {
    expect(truncate("hello", 10)).toBe("hello");
    expect(truncate("hello", 5)).toBe("hello");
  });

  it("truncates and appends ellipsis when over limit", () => {
    expect(truncate("hello world", 5)).toBe("hello\u2026");
  });
});

// ── snippetAround ──

describe("snippetAround", () => {
  it("returns text as-is when short enough", () => {
    expect(snippetAround("hello world", "world", 50)).toBe("hello world");
  });

  it("centers snippet around query", () => {
    const text = "a".repeat(100) + "TARGET" + "b".repeat(100);
    const snippet = snippetAround(text, "TARGET", 20);
    expect(snippet).toContain("TARGET");
    expect(snippet.length).toBeLessThanOrEqual(22); // maxLen + possible ellipsis chars
  });

  it("falls back to prefix when query not found", () => {
    const long = "x".repeat(200);
    const snippet = snippetAround(long, "MISSING", 10);
    expect(snippet.length).toBeLessThanOrEqual(11);
  });

  it("collapses whitespace", () => {
    const snippet = snippetAround("a\n\nb\t\tc", "b", 50);
    expect(snippet).not.toContain("\n");
    expect(snippet).not.toContain("\t");
  });
});

// ── formatBytes ──

describe("formatBytes", () => {
  it("shows bytes below 1 KB", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });

  it("shows KB for 1 KB to <1 MB", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1536)).toBe("1.5 KB");
  });

  it("shows MB for >= 1 MB", () => {
    expect(formatBytes(1024 * 1024)).toBe("1.0 MB");
    expect(formatBytes(1024 * 1024 * 2.5)).toBe("2.5 MB");
  });
});

// ── formatPasteSize ──

describe("formatPasteSize", () => {
  it("shows chars for single-line content", () => {
    expect(formatPasteSize(1, 50)).toBe("50 chars");
    expect(formatPasteSize(0, 80)).toBe("80 chars");
  });

  it("shows lines for multi-line content", () => {
    expect(formatPasteSize(5, 200)).toBe("5 lines");
    expect(formatPasteSize(999, 5000)).toBe("999 lines");
  });

  it("shows k-lines for >= 1000 lines", () => {
    expect(formatPasteSize(1000, 10000)).toBe("1.0k lines");
    expect(formatPasteSize(2500, 50000)).toBe("2.5k lines");
  });
});

// ── splitPath ──

describe("splitPath", () => {
  it("splits unix paths on /", () => {
    expect(splitPath("/home/dallas/code")).toEqual(["", "home", "dallas", "code"]);
  });

  it("splits windows paths on backslash", () => {
    expect(splitPath("C:\\Users\\dallas\\code")).toEqual(["C:", "Users", "dallas", "code"]);
  });

  it("handles mixed separators", () => {
    expect(splitPath("C:/Users\\dallas")).toEqual(["C:", "Users", "dallas"]);
  });
});

// ── fileName ──

describe("fileName", () => {
  it("extracts last segment from unix path", () => {
    expect(fileName("/home/dallas/foo.ts")).toBe("foo.ts");
  });

  it("extracts last segment from windows path", () => {
    expect(fileName("C:\\Users\\dallas\\foo.ts")).toBe("foo.ts");
  });

  it("returns the string itself for a bare filename", () => {
    expect(fileName("README.md")).toBe("README.md");
  });
});

// ── isAbsolutePath ──

describe("isAbsolutePath", () => {
  it("recognizes unix absolute paths", () => {
    expect(isAbsolutePath("/home/dallas")).toBe(true);
    expect(isAbsolutePath("~/projects")).toBe(true);
  });

  it("recognizes windows drive paths", () => {
    expect(isAbsolutePath("C:\\Users")).toBe(true);
    expect(isAbsolutePath("D:/code")).toBe(true);
  });

  it("recognizes UNC paths", () => {
    expect(isAbsolutePath("\\\\server\\share")).toBe(true);
  });

  it("rejects relative paths", () => {
    expect(isAbsolutePath("src/lib")).toBe(false);
    expect(isAbsolutePath("./foo")).toBe(false);
  });
});

// ── cwdDisplayLabel ──

describe("cwdDisplayLabel", () => {
  it("returns last path segment", () => {
    expect(cwdDisplayLabel("/home/dallas/myproject")).toBe("myproject");
  });

  it("returns / for root", () => {
    expect(cwdDisplayLabel("/")).toBe("/");
  });

  it("strips trailing slash before extracting segment", () => {
    expect(cwdDisplayLabel("/home/dallas/myproject/")).toBe("myproject");
  });

  it("returns / for empty string", () => {
    expect(cwdDisplayLabel("")).toBe("/");
  });
});

// ── formatInstallCount ──

describe("formatInstallCount", () => {
  it("shows raw count below 1k", () => {
    expect(formatInstallCount(0)).toBe("0");
    expect(formatInstallCount(999)).toBe("999");
  });

  it("shows K suffix for thousands", () => {
    expect(formatInstallCount(1000)).toBe("1K");
    expect(formatInstallCount(160242)).toBe("160K");
  });

  it("shows M suffix for millions", () => {
    expect(formatInstallCount(1_000_000)).toBe("1.0M");
    expect(formatInstallCount(2_500_000)).toBe("2.5M");
  });
});
