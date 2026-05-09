package main

import (
	"os"
)

type Config struct {
	DATABASE_URL string
	SSO_URL      string
}

func GetConfig() Config {
	conf := Config{
		DATABASE_URL: os.Getenv("DATABASE_URL"),
		SSO_URL:      os.Getenv("SSO_URL"),
	}

	return conf
}
