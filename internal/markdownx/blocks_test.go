package markdownx

import (
	"bytes"
	"strings"
	"testing"

	"github.com/yuin/goldmark"
	"github.com/yuin/goldmark/parser"
	"github.com/yuin/goldmark/renderer"
	"github.com/yuin/goldmark/util"
)

func render(t *testing.T, src string) string {
	t.Helper()
	md := goldmark.New(
		goldmark.WithParserOptions(
			parser.WithBlockParsers(util.Prioritized(NewAdmonitionParser(), 50)),
		),
		goldmark.WithRendererOptions(
			renderer.WithNodeRenderers(util.Prioritized(NewAdmonitionRenderer(), 50)),
		),
	)
	var buf bytes.Buffer
	if err := md.Convert([]byte(src), &buf); err != nil {
		t.Fatal(err)
	}
	return buf.String()
}

func TestAdmonitionBlocks(t *testing.T) {
	got := render(t, ":::warning 小心\n\n内容\n\n:::\n")
	if !strings.Contains(got, `<div class="admonition warning">`) {
		t.Fatalf("missing warning div: %s", got)
	}
	if !strings.Contains(got, `admonition-title">小心</p>`) {
		t.Fatalf("missing title: %s", got)
	}
	if !strings.Contains(got, "</div>") {
		t.Fatalf("missing close: %s", got)
	}

	got = render(t, ":::info\n\n提示\n\n:::\n")
	if !strings.Contains(got, `<div class="admonition info">`) {
		t.Fatalf("info: %s", got)
	}

	got = render(t, ":::details 更多\n\n隐藏内容\n\n:::\n")
	if !strings.Contains(got, "<details><summary>更多</summary>") ||
		!strings.Contains(got, "隐藏内容") || !strings.Contains(got, "</details>") {
		t.Fatalf("details: %s", got)
	}
}

func TestUnclosedOrUnknownKind(t *testing.T) {
	// Unknown kind falls through to plain text.
	got := render(t, ":::bogus x\n\ny\n")
	if strings.Contains(got, "admonition") {
		t.Fatalf("unknown kind rendered: %s", got)
	}
	// Unclosed block still renders its content.
	got = render(t, ":::warning\n\n内容\n")
	if !strings.Contains(got, "admonition warning") || !strings.Contains(got, "内容") {
		t.Fatalf("unclosed: %s", got)
	}
	// Escaping of title.
	got = render(t, ":::warning <b>&x\n\nc\n\n:::\n")
	if strings.Contains(got, "<b>") {
		t.Fatalf("title not escaped: %s", got)
	}
}
