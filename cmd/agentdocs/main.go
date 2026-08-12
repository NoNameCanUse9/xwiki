package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"syscall"

	"agentdocs/internal/app"
	"agentdocs/internal/config"
	"agentdocs/internal/maintenance"
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
	case "backup":
		return backup(args[1:])
	case "restore":
		return restore(args[1:])
	case "doctor":
		return doctor(args[1:])
	case "help", "-h", "--help":
		fmt.Fprint(os.Stdout, usageText)
		return nil
	default:
		return fmt.Errorf("unknown command %q", args[0])
	}
}

func backup(args []string) error {
	fs := flag.NewFlagSet("backup", flag.ContinueOnError)
	dataDir := fs.String("data-dir", "", "data directory")
	output := fs.String("output", "", "backup output path")
	if err := fs.Parse(args); err != nil {
		return err
	}
	cfg := config.Load()
	if *dataDir != "" {
		cfg.DataDir = *dataDir
	}
	if *output == "" {
		return errors.New("backup --output is required")
	}
	return maintenance.Backup(context.Background(), cfg.DataDir, *output)
}

func restore(args []string) error {
	fs := flag.NewFlagSet("restore", flag.ContinueOnError)
	dataDir := fs.String("data-dir", "", "data directory")
	replace := fs.Bool("replace", false, "replace an existing data directory")
	if err := fs.Parse(args); err != nil {
		return err
	}
	if fs.NArg() != 1 {
		return errors.New("usage: agentdocs restore [--data-dir DIR] [--replace] <backup.tar.gz>")
	}
	cfg := config.Load()
	if *dataDir != "" {
		cfg.DataDir = *dataDir
	}
	return maintenance.Restore(context.Background(), fs.Arg(0), cfg.DataDir, *replace)
}

func doctor(args []string) error {
	fs := flag.NewFlagSet("doctor", flag.ContinueOnError)
	dataDir := fs.String("data-dir", "", "data directory")
	jsonOutput := fs.Bool("json", false, "print JSON report")
	if err := fs.Parse(args); err != nil {
		return err
	}
	cfg := config.Load()
	if *dataDir != "" {
		cfg.DataDir = *dataDir
	}
	report, err := maintenance.Doctor(context.Background(), cfg.DataDir)
	if err != nil {
		return err
	}
	if *jsonOutput {
		return json.NewEncoder(os.Stdout).Encode(report)
	}
	for _, check := range report.Checks {
		fmt.Printf("%-16s %-7s %s\n", check.Name, check.Status, check.Detail)
	}
	if !report.Healthy {
		return errors.New("doctor found unhealthy checks")
	}
	return nil
}

// reindex rebuilds the full-text index for all projects (or one with --project).
func reindex(args []string) error {
	fs := flag.NewFlagSet("reindex", flag.ExitOnError)
	dataDir := fs.String("data-dir", "", "data directory (default: $AGENTDOCS_DATA_DIR or data)")
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
	dataDir := fs.String("data-dir", "", "data directory (default: $AGENTDOCS_DATA_DIR or data)")
	httpAddr := fs.String("http-addr", "", "HTTP listen address (default: $AGENTDOCS_HTTP_ADDR or :8080)")
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
		return errors.New("usage: agentdocs admin create -username <name> [-password <pw>]")
	}
	fs := flag.NewFlagSet("admin create", flag.ExitOnError)
	username := fs.String("username", "", "admin username")
	password := fs.String("password", "", "admin password (fallback: $AGENTDOCS_ADMIN_PASSWORD)")
	_ = fs.Parse(args[1:])
	if *username == "" {
		return errors.New("username is required")
	}
	pw := *password
	if pw == "" {
		pw = os.Getenv("AGENTDOCS_ADMIN_PASSWORD")
	}
	if pw == "" {
		return errors.New("password is required (flag -password or env AGENTDOCS_ADMIN_PASSWORD)")
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

const usageText = `AgentDocs - Git-backed documentation server for humans and AI agents

Usage:
  agentdocs serve              start the HTTP server
  agentdocs admin create       create the first administrator user
  agentdocs backup --output F  create an offline data backup
  agentdocs restore FILE       restore an offline data backup
  agentdocs doctor             diagnose the data directory
  agentdocs reindex            rebuild the search index
  agentdocs help               show this help
`
