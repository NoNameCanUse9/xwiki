package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"syscall"

	"xwiki/internal/app"
	"xwiki/internal/config"
)

func main() {
	slog.SetDefault(slog.New(slog.NewJSONHandler(os.Stderr, nil)))
	if err := run(os.Args[1:]); err != nil {
		slog.Error("command failed", "error", err)
		os.Exit(1)
	}
}

func run(args []string) error {
	if len(args) == 0 {
		return usageError()
	}
	switch args[0] {
	case "serve":
		return serve(args[1:])
	case "admin":
		return admin(args[1:])
	case "reindex":
		return reindex(args[1:])
	case "help", "-h", "--help":
		fmt.Fprint(os.Stdout, usageText)
		return nil
	default:
		return fmt.Errorf("unknown command %q", args[0])
	}
}

// reindex rebuilds the full-text index for all projects (or one with --project).
func reindex(args []string) error {
	fs := flag.NewFlagSet("reindex", flag.ExitOnError)
	dataDir := fs.String("data-dir", "", "data directory (default: $XWIKI_DATA_DIR or data)")
	projectID := fs.String("project", "", "reindex only this project id")
	_ = fs.Parse(args)

	cfg := config.Load()
	if *dataDir != "" {
		cfg.DataDir = *dataDir
	}
	a, err := app.New(cfg)
	if err != nil {
		return err
	}
	defer a.Close()

	ctx := context.Background()
	if *projectID != "" {
		stats, err := a.SearchSvc().ReindexProject(ctx, *projectID)
		if err != nil {
			return fmt.Errorf("reindex %s: %w", *projectID, err)
		}
		fmt.Printf("reindexed %s: %d indexed, %d removed\n", *projectID, stats.Indexed, stats.Removed)
		return nil
	}
	all, err := a.SearchSvc().ReindexAll(ctx)
	if err != nil {
		return err
	}
	total := 0
	for id, stats := range all {
		fmt.Printf("%s: %d indexed, %d removed\n", id, stats.Indexed, stats.Removed)
		total += stats.Indexed
	}
	fmt.Printf("done: %d projects, %d documents indexed\n", len(all), total)
	return nil
}

func serve(args []string) error {
	fs := flag.NewFlagSet("serve", flag.ExitOnError)
	dataDir := fs.String("data-dir", "", "data directory (default: $XWIKI_DATA_DIR or data)")
	httpAddr := fs.String("http-addr", "", "HTTP listen address (default: $XWIKI_HTTP_ADDR or :8080)")
	_ = fs.Parse(args)

	cfg := config.Load()
	if *dataDir != "" {
		cfg.DataDir = *dataDir
	}
	if *httpAddr != "" {
		cfg.HTTPAddr = *httpAddr
	}

	a, err := app.New(cfg)
	if err != nil {
		return err
	}
	defer a.Close()

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	return a.Run(ctx)
}

func admin(args []string) error {
	if len(args) == 0 || args[0] != "create" {
		return errors.New("usage: xwiki admin create -username <name> [-password <pw>]")
	}
	fs := flag.NewFlagSet("admin create", flag.ExitOnError)
	username := fs.String("username", "", "admin username")
	password := fs.String("password", "", "admin password (fallback: $XWIKI_ADMIN_PASSWORD)")
	_ = fs.Parse(args[1:])
	if *username == "" {
		return errors.New("username is required")
	}
	pw := *password
	if pw == "" {
		pw = os.Getenv("XWIKI_ADMIN_PASSWORD")
	}
	if pw == "" {
		return errors.New("password is required (flag -password or env XWIKI_ADMIN_PASSWORD)")
	}
	a, err := app.New(config.Load())
	if err != nil {
		return err
	}
	defer a.Close()
	return a.CreateAdmin(context.Background(), *username, pw)
}

func usageError() error {
	fmt.Fprint(os.Stdout, usageText)
	return errors.New("missing command")
}

const usageText = `XWiki - Git-backed documentation server for humans and AI agents

Usage:
  xwiki serve              start the HTTP server
  xwiki admin create       create the first administrator user
  xwiki help               show this help
`
