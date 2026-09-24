from flask import Flask

app = Flask(__name__)


@app.route("/", defaults={"path": ""}, methods=["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"])
@app.route("/<path:path>", methods=["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"])
def catch_all(path):
    return "OK", 200


if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000)
