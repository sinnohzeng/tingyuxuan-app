import { describe, it, expect } from "vitest";
import { renderMarkdown } from "../shared/lib/markdown";

describe("renderMarkdown", () => {
  it("renders bold text", () => {
    const html = renderMarkdown("**bold**");
    expect(html).toContain("<strong>bold</strong>");
  });

  it("renders italic text", () => {
    const html = renderMarkdown("*italic*");
    expect(html).toContain("<em>italic</em>");
  });

  it("renders inline code", () => {
    const html = renderMarkdown("`code`");
    expect(html).toContain("<code>code</code>");
  });

  it("renders unordered lists", () => {
    const html = renderMarkdown("- item 1\n- item 2");
    expect(html).toContain("<ul>");
    expect(html).toContain("<li>item 1</li>");
    expect(html).toContain("<li>item 2</li>");
    expect(html).toContain("</ul>");
  });

  it("renders ordered lists", () => {
    const html = renderMarkdown("1. first\n2. second");
    expect(html).toContain("<ol>");
    expect(html).toContain("<li>first</li>");
    expect(html).toContain("<li>second</li>");
    expect(html).toContain("</ol>");
  });

  it("renders headings", () => {
    const html = renderMarkdown("# Title");
    expect(html).toContain("<h3>Title</h3>");
  });

  it("escapes HTML to prevent XSS", () => {
    const html = renderMarkdown("<script>alert('xss')</script>");
    expect(html).not.toContain("<script>");
    expect(html).toContain("&lt;script&gt;");
  });

  it("renders plain paragraphs", () => {
    const html = renderMarkdown("Hello world");
    expect(html).toContain("<p>Hello world</p>");
  });
});
