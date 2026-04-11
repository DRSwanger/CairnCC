import { Marked } from "marked";
import { escapeHtml } from "$lib/utils/ansi";
import hljs from "highlight.js/lib/core";
import mermaid from "mermaid";
import javascript from "highlight.js/lib/languages/javascript";
import typescript from "highlight.js/lib/languages/typescript";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import bash from "highlight.js/lib/languages/bash";
import json from "highlight.js/lib/languages/json";
import css from "highlight.js/lib/languages/css";
import xml from "highlight.js/lib/languages/xml";
import markdown from "highlight.js/lib/languages/markdown";
import yaml from "highlight.js/lib/languages/yaml";
import sql from "highlight.js/lib/languages/sql";
import go from "highlight.js/lib/languages/go";
import java from "highlight.js/lib/languages/java";
import cpp from "highlight.js/lib/languages/cpp";
import diff from "highlight.js/lib/languages/diff";
import shell from "highlight.js/lib/languages/shell";
import DOMPurify from "dompurify";
import "highlight.js/styles/github-dark.min.css";

// Register languages with common aliases
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("js", javascript);
hljs.registerLanguage("typescript", typescript);
hljs.registerLanguage("ts", typescript);
hljs.registerLanguage("python", python);
hljs.registerLanguage("py", python);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("rs", rust);
hljs.registerLanguage("bash", bash);
hljs.registerLanguage("sh", bash);
hljs.registerLanguage("json", json);
hljs.registerLanguage("css", css);
hljs.registerLanguage("html", xml);
hljs.registerLanguage("xml", xml);
hljs.registerLanguage("markdown", markdown);
hljs.registerLanguage("md", markdown);
hljs.registerLanguage("yaml", yaml);
hljs.registerLanguage("yml", yaml);
hljs.registerLanguage("sql", sql);
hljs.registerLanguage("go", go);
hljs.registerLanguage("java", java);
hljs.registerLanguage("cpp", cpp);
hljs.registerLanguage("c", cpp);
hljs.registerLanguage("diff", diff);
hljs.registerLanguage("shell", shell);

mermaid.initialize({
  startOnLoad: false,
  theme: "dark",
  themeVariables: {
    primaryColor: "#f97316",
    primaryTextColor: "#e5e7eb",
    primaryBorderColor: "#374151",
    lineColor: "#6b7280",
    sectionBkgColor: "#1f2937",
    altSectionBkgColor: "#111827",
    gridColor: "#374151",
    secondaryColor: "#1f2937",
    tertiaryColor: "#111827",
    background: "#0f1117",
    mainBkg: "#1a1d27",
    nodeBorder: "#374151",
    clusterBkg: "#1f2937",
    titleColor: "#f97316",
    edgeLabelBackground: "#1f2937",
    attributeBackgroundColorEven: "#1f2937",
    attributeBackgroundColorOdd: "#111827",
  },
});

let mermaidCounter = 0;

const marked = new Marked();

marked.use({
  gfm: true,
  breaks: false,
  renderer: {
    // marked v15: table(token) receives a Token with header[] and rows[][]
    table(token: {
      header: Array<{ tokens: unknown[]; align: string | null; header: boolean }>;
      rows: Array<Array<{ tokens: unknown[]; align: string | null; header: boolean }>>;
    }) {
      // Build header cells
      let headerCells = "";
      for (const cell of token.header) {
        const content = this.parser.parseInline(cell.tokens);
        const tag = cell.align ? `<th align="${cell.align}">` : "<th>";
        headerCells += `${tag}${content}</th>\n`;
      }
      const headerRow = `<tr>\n${headerCells}</tr>\n`;

      // Build body rows
      let body = "";
      for (const row of token.rows) {
        let rowCells = "";
        for (const cell of row) {
          const content = this.parser.parseInline(cell.tokens);
          const tag = cell.align ? `<td align="${cell.align}">` : "<td>";
          rowCells += `${tag}${content}</td>\n`;
        }
        body += `<tr>\n${rowCells}</tr>\n`;
      }
      if (body) body = `<tbody>${body}</tbody>`;

      return `<div class="table-wrapper"><table><thead>${headerRow}</thead>${body}</table></div>`;
    },
    code({ text, lang }: { text: string; lang?: string }) {
      const language = (lang || "").toLowerCase();

      // Mermaid diagrams — render as a placeholder, processed after mount
      if (language === "mermaid") {
        const id = `mermaid-${++mermaidCounter}`;
        return `<div class="mermaid-block" data-mermaid-id="${id}"><div class="mermaid-pending" id="${id}">${escapeHtml(text)}</div></div>`;
      }

      let highlighted: string;
      if (language && hljs.getLanguage(language)) {
        try {
          highlighted = hljs.highlight(text, { language }).value;
        } catch {
          highlighted = escapeHtml(text);
        }
      } else {
        highlighted = escapeHtml(text);
      }

      const displayLang = language || "text";
      return `<div class="code-block"><div class="code-block-header"><span class="code-block-lang">${escapeHtml(displayLang)}</span><button class="code-block-copy" data-code-copy>Copy</button></div><pre><code class="hljs language-${escapeHtml(language)}">${highlighted}</code></pre></div>`;
    },
  },
});

export function renderMarkdown(text: string): string {
  const raw = marked.parse(text);
  if (typeof raw !== "string") {
    return "";
  }
  return DOMPurify.sanitize(raw, {
    ADD_ATTR: ["class", "target", "data-code-copy", "data-mermaid-id", "id"],
    ADD_TAGS: ["div"],
  });
}

export async function renderMermaidBlocks(container: HTMLElement): Promise<void> {
  const blocks = container.querySelectorAll<HTMLElement>(".mermaid-pending");
  for (const block of blocks) {
    const id = block.id;
    const source = block.textContent || "";
    if (!source.trim()) continue;
    try {
      const { svg } = await mermaid.render(`mermaid-svg-${id}`, source);
      const wrapper = block.parentElement;
      if (wrapper) {
        wrapper.innerHTML = svg;
        wrapper.classList.add("mermaid-rendered");
      }
    } catch {
      block.classList.add("mermaid-error");
      block.textContent = source;
    }
  }
}
