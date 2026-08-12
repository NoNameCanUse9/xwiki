package app

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"log/slog"
	"net/http"
	"strings"
	"time"

	"agentdocs/internal/agent"
	"agentdocs/internal/auth"
	"agentdocs/internal/config"
	"agentdocs/internal/maintenance"
	"agentdocs/internal/platform/clock"
	"agentdocs/internal/platform/id"
	"agentdocs/internal/project"
	"agentdocs/internal/search"
	"agentdocs/internal/server"
	"agentdocs/internal/store/sqlite"
	"agentdocs/internal/user"
)

// App wires configuration, storage, services and the HTTP handler.
type App struct {
	cfg         *config.Config
	log         *slog.Logger
	db          *sql.DB
	dataLock    *maintenance.DataLock
	clock       clock.Clock
	users       *user.Store
	authSvc     *auth.Service
	projectsSvc *project.Service
	searchSvc   *search.Service
	handler     http.Handler
}

func New(cfg *config.Config) (*App, error) {
	log := slog.Default()
	dataLock, err := maintenance.AcquireDataLock(cfg.DataDir)
	if err != nil {
		return nil, err
	}
	db, err := sqlite.Open(cfg.DataDir)
	if err != nil {
		_ = dataLock.Close()
		return nil, fmt.Errorf("open sqlite: %w", err)
	}
	clk := clock.Real{}
	users := user.NewStore(db)
	authSvc := auth.NewService(db, clk, cfg.SessionTTL)
	projectsSvc := project.NewService(db, cfg.DataDir, clk)
	agentSvc := agent.NewService(db, clk)
	searchSvc := search.NewService(db, projectsSvc)
	handler := server.NewRouter(cfg, log, db, users, authSvc, projectsSvc, agentSvc, searchSvc)
	return &App{
		cfg: cfg, log: log, db: db, dataLock: dataLock, clock: clk,
		users: users, authSvc: authSvc, projectsSvc: projectsSvc, searchSvc: searchSvc, handler: handler,
	}, nil
}

func (a *App) Handler() http.Handler { return a.handler }

// SearchSvc exposes the search service for CLI commands.
func (a *App) SearchSvc() *search.Service { return a.searchSvc }

func (a *App) Close() error {
	var dbErr error
	if a.db != nil {
		dbErr = a.db.Close()
		a.db = nil
	}
	var lockErr error
	if a.dataLock != nil {
		lockErr = a.dataLock.Close()
		a.dataLock = nil
	}
	return errors.Join(dbErr, lockErr)
}

// CreateAdmin creates the first administrator user (idempotency: duplicate
// username is an error).
func (a *App) CreateAdmin(ctx context.Context, username, password string) error {
	username = strings.TrimSpace(username)
	if len(username) < 3 {
		return errors.New("username must be at least 3 characters")
	}
	if len(password) < 8 {
		return errors.New("password must be at least 8 characters")
	}
	if _, err := a.users.GetByUsername(ctx, username); err == nil {
		return fmt.Errorf("user %q already exists", username)
	} else if !errors.Is(err, user.ErrNotFound) {
		return err
	}
	hash, err := auth.HashPassword(password)
	if err != nil {
		return err
	}
	now := a.clock.Now().UTC()
	u := &user.User{
		ID: id.New("usr"), Username: username, DisplayName: username,
		PasswordHash: hash, IsAdmin: true, CreatedAt: now, UpdatedAt: now,
	}
	return a.users.Create(ctx, u)
}

// Run serves HTTP until ctx is cancelled, then shuts down gracefully.
func (a *App) Run(ctx context.Context) error {
	srv := &http.Server{
		Addr:              a.cfg.HTTPAddr,
		Handler:           a.handler,
		ReadHeaderTimeout: 5 * time.Second,
	}
	errCh := make(chan error, 1)
	go func() { errCh <- srv.ListenAndServe() }()
	a.log.Info("server started", "addr", a.cfg.HTTPAddr)
	select {
	case err := <-errCh:
		return err
	case <-ctx.Done():
		shCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		return srv.Shutdown(shCtx)
	}
}
