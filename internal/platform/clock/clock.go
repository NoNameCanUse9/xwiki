package clock

import "time"

// Clock abstracts time so services are testable with a fake clock.
type Clock interface {
	Now() time.Time
}

type Real struct{}

func (Real) Now() time.Time { return time.Now() }
