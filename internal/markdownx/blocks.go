// Package markdownx extends goldmark with XWiki-specific block syntax:
//
//	:::info 标题
//	内容...
//	:::
//
// Supported kinds: info, warning, danger, details (details renders a
// <details><summary> collapsible block; the title becomes the summary).
package markdownx

import (
	"bytes"

	"github.com/yuin/goldmark/ast"
	"github.com/yuin/goldmark/parser"
	"github.com/yuin/goldmark/renderer"
	"github.com/yuin/goldmark/text"
	"github.com/yuin/goldmark/util"
)

// Admonition is a container block opened by ":::kind [title]" and closed by
// a line containing only ":::".
type Admonition struct {
	ast.BaseBlock
	Label string // info | warning | danger | details
	Title string
}

// Kind implements ast.Node.
func (a *Admonition) Kind() ast.NodeKind { return admonitionKind }

// Dump implements ast.Node.
func (a *Admonition) Dump(source []byte, level int) {
	ast.DumpHelper(a, source, level, map[string]string{
		"Kind":  a.Label,
		"Title": a.Title,
	}, nil)
}

var openPattern = []byte(":::")

type admonitionParser struct{}

// NewAdmonitionParser returns the block parser for ::: containers.
func NewAdmonitionParser() parser.BlockParser { return &admonitionParser{} }

func (p *admonitionParser) Trigger() []byte { return openPattern }

func (p *admonitionParser) Open(parent ast.Node, reader text.Reader, pc parser.Context) (ast.Node, parser.State) {
	line, _ := reader.PeekLine()
	trimmed := bytes.TrimSpace(line)
	if !bytes.HasPrefix(trimmed, openPattern) {
		return nil, parser.NoChildren
	}
	rest := bytes.TrimSpace(trimmed[len(openPattern):])
	if len(rest) == 0 {
		return nil, parser.NoChildren
	}
	// kind is the first whitespace-delimited token; the remainder is the title.
	fields := bytes.Fields(rest)
	label := string(fields[0])
	switch label {
	case "info", "warning", "danger", "details":
	default:
		return nil, parser.NoChildren
	}
	title := ""
	if len(fields) > 1 {
		title = string(bytes.Join(fields[1:], []byte(" ")))
	}
	reader.Advance(len(line))
	node := &Admonition{Label: label, Title: title}
	return node, parser.HasChildren
}

func (p *admonitionParser) Continue(node ast.Node, reader text.Reader, pc parser.Context) parser.State {
	line, _ := reader.PeekLine()
	if bytes.Equal(bytes.TrimSpace(line), openPattern) {
		reader.Advance(len(line))
		return parser.Close
	}
	return parser.Continue | parser.HasChildren
}

func (p *admonitionParser) Close(node ast.Node, reader text.Reader, pc parser.Context) {
	// Children were parsed by the paragraph/child parsers.
}

func (p *admonitionParser) CanInterruptParagraph() bool { return true }

func (p *admonitionParser) CanAcceptIndentedLine() bool { return false }

// admonitionRenderer emits the container HTML.
type admonitionRenderer struct{}

// NewAdmonitionRenderer returns the renderer for ::: containers.
func NewAdmonitionRenderer() renderer.NodeRenderer { return &admonitionRenderer{} }

func (r *admonitionRenderer) RegisterFuncs(reg renderer.NodeRendererFuncRegisterer) {
	reg.Register(admonitionKind, r.render)
}

var admonitionKind = ast.NewNodeKind("Admonition")

func (r *admonitionRenderer) render(w util.BufWriter, source []byte, node ast.Node, entering bool) (ast.WalkStatus, error) {
	a, ok := node.(*Admonition)
	if !ok {
		return ast.WalkStop, nil
	}
	if entering {
		if a.Label == "details" {
			w.WriteString("<details")
			if a.Title != "" {
				w.WriteString("><summary>")
				w.Write(escapeHTML([]byte(a.Title)))
				w.WriteString("</summary>")
			} else {
				w.WriteString(">")
			}
		} else {
			w.WriteString(`<div class="admonition `)
			w.WriteString(a.Label)
			w.WriteString(`">`)
			if a.Title != "" {
				w.WriteString(`<p class="admonition-title">`)
				w.Write(escapeHTML([]byte(a.Title)))
				w.WriteString("</p>")
			}
		}
		return ast.WalkContinue, nil
	}
	if a.Label == "details" {
		w.WriteString("</details>")
	} else {
		w.WriteString("</div>")
	}
	return ast.WalkContinue, nil
}

func escapeHTML(b []byte) []byte {
	var out []byte
	for _, c := range b {
		switch c {
		case '&':
			out = append(out, "&amp;"...)
		case '<':
			out = append(out, "&lt;"...)
		case '>':
			out = append(out, "&gt;"...)
		default:
			out = append(out, c)
		}
	}
	return out
}

// NodeRendererFuncRegisterer-compatible registration: the renderer is added
// via renderer.WithNodeRenderers in the application.
var _ renderer.NodeRenderer = &admonitionRenderer{}
var _ parser.BlockParser = &admonitionParser{}
