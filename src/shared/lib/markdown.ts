/**
 * Lightweight Markdown renderer for AI assistant responses.
 * Converts a subset of Markdown to HTML. XSS-safe: HTML entities are escaped
 * before any Markdown transformations are applied.
 */

/** Escape HTML special characters to prevent XSS. */
function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Render inline Markdown: bold, italic, code. */
function renderInline(line: string): string {
  return (
    line
      // Inline code: `code`
      .replace(/`([^`]+)`/g, "<code>$1</code>")
      // Bold: **text**
      .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
      // Italic: *text* (but not inside **)
      .replace(/(?<!\*)\*([^*]+)\*(?!\*)/g, "<em>$1</em>")
  );
}

/** Render a Markdown string to sanitized HTML. */
export function renderMarkdown(input: string): string {
  const escaped = escapeHtml(input);
  const lines = escaped.split("\n");
  const output: string[] = [];

  let inUl = false;
  let inOl = false;

  const closeList = () => {
    if (inUl) {
      output.push("</ul>");
      inUl = false;
    }
    if (inOl) {
      output.push("</ol>");
      inOl = false;
    }
  };

  for (const line of lines) {
    const trimmed = line.trim();

    // Empty line → break + close any open list.
    if (!trimmed) {
      closeList();
      output.push("<br/>");
      continue;
    }

    // Heading: # Title
    const headingMatch = trimmed.match(/^(#{1,3})\s+(.+)$/);
    if (headingMatch) {
      closeList();
      const level = headingMatch[1].length + 2; // # → h3, ## → h4, ### → h5
      const capped = Math.min(level, 6);
      output.push(`<h${capped}>${renderInline(headingMatch[2])}</h${capped}>`);
      continue;
    }

    // Unordered list: - item or * item
    const ulMatch = trimmed.match(/^[-*]\s+(.+)$/);
    if (ulMatch) {
      if (inOl) {
        output.push("</ol>");
        inOl = false;
      }
      if (!inUl) {
        output.push("<ul>");
        inUl = true;
      }
      output.push(`<li>${renderInline(ulMatch[1])}</li>`);
      continue;
    }

    // Ordered list: 1. item
    const olMatch = trimmed.match(/^\d+\.\s+(.+)$/);
    if (olMatch) {
      if (inUl) {
        output.push("</ul>");
        inUl = false;
      }
      if (!inOl) {
        output.push("<ol>");
        inOl = true;
      }
      output.push(`<li>${renderInline(olMatch[1])}</li>`);
      continue;
    }

    // Regular paragraph line.
    closeList();
    output.push(`<p>${renderInline(trimmed)}</p>`);
  }

  closeList();
  return output.join("");
}
