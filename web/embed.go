// Package web embeds the built frontend so the Go binary is self-contained.
package web

import "embed"

// Dist contains the built frontend (web/dist). The committed placeholder
// index.html keeps go:embed working on fresh checkouts; `npm run build`
// replaces it with the real bundle.
//
//go:embed dist
var Dist embed.FS
