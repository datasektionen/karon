package main

import (
	"net/http"
	"text/template"

	_ "github.com/jmoiron/sqlx"
	_ "github.com/lib/pq"
)

// func hello(w http.ResponseWriter, req *http.Request) {
// 	fmt.Fprintf(w, "hello\n")
// }
//
// func headers(w http.ResponseWriter, req *http.Request) {
// 	for name, headers := range req.Header {
// 		for _, h := range headers {
// 			fmt.Fprintf(w, "%v: %v\n", name, h)
// 		}
// 	}
// }

func main() {
	// db, err := sqlx.Connect("postgres", "user=foo dbname=bar sslmode=disable")
	// if err != nil {
	// 	log.Fatalln(err)
	// }
	//
	// db.MustExec("dldl")

	fs := http.FileServer(http.Dir("./client/public"))
	http.Handle("/public/", http.StripPrefix("/public/", fs))

	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		tmpl, err := template.ParseFiles(
			"./client/templates/home.html",
			"./client/templates/header.html",
		)
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}

		tmpl.Execute(w, nil)
	})

	println("Server running at http://localhost:8080")
	http.ListenAndServe(":8080", nil)
}
