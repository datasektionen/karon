package server

import (
	"embed"
	"io/fs"
	"net/http"
	"text/template"
)

func Serve(files embed.FS) {
	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		tmpl, err := template.ParseFS(files, "client/templates/home.html", "client/templates/header.html")
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		tmpl.Execute(w, nil)
	})

	publicFolder, _ := fs.Sub(files, "client/public")
	http.Handle("/public/", http.StripPrefix("/public/", http.FileServer(http.FS(publicFolder))))

	http.ListenAndServe(":8080", nil)
}
