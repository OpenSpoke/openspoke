package main

import "crypto/rand"

// cryptoRandRead is a thin wrapper over crypto/rand.Read.
// Only called from cryptoRandReader in main.go.
func cryptoRandRead(p []byte) (int, error) {
	return rand.Read(p)
}
