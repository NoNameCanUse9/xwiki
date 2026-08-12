package project

import (
	"errors"
	"regexp"
	"strings"
	"time"
)

// Project is the metadata record for one Git-backed documentation project.
type Project struct {
	ID          string     `json:"id"`
	Name        string     `json:"name"`
	Description string     `json:"description"`
	RepoDir     string     `json:"repo_dir"`
	Archived    bool       `json:"archived"`
	ArchivedAt  *time.Time `json:"archived_at,omitempty"`
	Deleted     bool       `json:"deleted"`
	DeletedAt   *time.Time `json:"deleted_at,omitempty"`
	CreatedAt   time.Time  `json:"created_at"`
	UpdatedAt   time.Time  `json:"updated_at"`
}

// IsArchived reports whether the project has been archived.
func (p *Project) IsArchived() bool { return p.ArchivedAt != nil }

// IsDeleted reports whether the project is in the recoverable trash.
func (p *Project) IsDeleted() bool { return p.DeletedAt != nil }

// Sentinel errors returned by the store and service layers.
var (
	ErrNotFound   = errors.New("project not found")
	ErrConflict   = errors.New("project name already exists")
	ErrInvalid    = errors.New("invalid project name")
	ErrNotDeleted = errors.New("project is not deleted")
)

// namePattern matches the safe project-name grammar: lowercase letters,
// digits and single hyphens, never starting or ending with a hyphen.
var namePattern = regexp.MustCompile(`^[a-z0-9]+(-[a-z0-9]+)*$`)

// ValidateName checks the project-name grammar. Names are intentionally
// restricted so they are safe as Git repository directory names.
func ValidateName(name string) error {
	name = strings.TrimSpace(name)
	if name == "" || len(name) > 64 || !namePattern.MatchString(name) {
		return ErrInvalid
	}
	return nil
}
