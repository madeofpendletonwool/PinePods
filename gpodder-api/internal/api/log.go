package api

import (
	"log"
	"os"
)

// debugEnabled mirrors the stack-wide DEBUG_MODE flag. When false, debugf is a no-op
// so verbose per-request tracing stays out of production logs.
var debugEnabled = os.Getenv("DEBUG_MODE") == "true"

// debugf logs a "[DEBUG]" line only when DEBUG_MODE=true. Use it for per-request
// tracing; never pass secrets (auth tokens, cookies, passwords) to it.
func debugf(format string, args ...interface{}) {
	if debugEnabled {
		log.Printf("[DEBUG] "+format, args...)
	}
}
