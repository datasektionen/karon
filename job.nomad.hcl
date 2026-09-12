job "karon" {
  type = "service"

  group "karon" {
    network {
      port "http" { }
    }

    service {
      name     = "karon"
      port     = "http"
      provider = "nomad"
      tags = [
        "traefik.enable=true",
        "traefik.http.routers.karon.rule=Host(`karon.datasektionen.se`)",
        "traefik.http.routers.karon.tls.certresolver=default",
      ]
    }

    task "karon" {
      driver = "docker"

      config {
        image = var.image_tag
        ports = ["http"]
      }

      template {
        data        = <<ENV
{{ with nomadVar "nomad/jobs/karon" }}
APP_SECRET={{ .app_secret }}
OIDC_CLIENT_SECRET={{ .oidc_secret }}
HIVE_API_KEY={{ .hive_api_key }}
DATABASE_URL=postgresql://karon:{{ .database_password }}@postgres.dsekt.internal:5432/karon
{{ end }}
PORT={{ env "NOMAD_PORT_http" }}
OIDC_PROVIDER=https://sso.datasektionen.se/op
OIDC_CLIENT_ID=karon
OIDC_REDIRECT_URL=https://karon.datasektionen.se/auth/callback
SSO_URL=http://sso.nomad.dsekt.internal
HIVE_URL=https://hive.datasektionen.se/api/v1
VOTEIT_URL=https://ths.voteit.se
TZ=Europe/Stockholm
RUST_LOG=info
ENV
        destination = "local/.env"
        env         = true
      }

      resources {
        memory = 120
      }
    }
  }
}

variable "image_tag" {
  type = string
  default = "ghcr.io/datasektionen/karon:latest"
}
