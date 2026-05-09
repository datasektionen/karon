package main

import (
	"embed"

	"karon/server"

	"github.com/jmoiron/sqlx"
	_ "github.com/lib/pq"
	"github.com/pressly/goose/v3"
)

//go:embed client/templates/* client/public/*
var webAssets embed.FS

//go:embed database/migrations/*.sql
var migrations embed.FS

func main() {
	conf := GetConfig()

	db := sqlx.MustConnect("postgres", conf.DATABASE_URL)

	// Migrate database
	goose.SetBaseFS(migrations)
	if err := goose.SetDialect("postgres"); err != nil {
		panic(err)
	}
	if err := goose.Up(db.DB, "database/migrations"); err != nil {
		panic(err)
	}

	server.Serve(webAssets)
}
