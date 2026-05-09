package main

import (
	"embed"
	"net/http"
	"text/template"

	"github.com/jmoiron/sqlx"
	_ "github.com/lib/pq"
	"github.com/pressly/goose/v3"
)

//go:embed client/templates/* client/public/*
var files embed.FS

//go:embed database/migrations/*.sql
var embedMigrations embed.FS

func main() {
	conf := GetConfig()

	db := sqlx.MustConnect("postgres", conf.DATABASE_URL)
	goose.SetBaseFS(embedMigrations)
	if err := goose.SetDialect("postgres"); err != nil {
		panic(err)
	}
	if err := goose.Up(db.DB, "database/migrations"); err != nil {
		panic(err)
	}

	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		tmpl, err := template.ParseFS(files, "client/templates/home.html", "client/templates/header.html")
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		tmpl.Execute(w, nil)
	})

	publicFiles := http.FS(files)
	http.Handle("/public/", http.FileServer(publicFiles))

	http.ListenAndServe(":8080", nil)
}
