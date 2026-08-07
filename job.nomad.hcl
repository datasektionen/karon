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
OIDC_SECRET={{ .oidc_secret }}
HIVE_TOKEN={{ .hive_api_key }}
DATABASE_URL=postgresql://karon:{{ .database_password }}@postgres.dsekt.internal:5432/karon
{{ end }}
PORT={{ env "NOMAD_PORT_http" }}
OIDC_PROVIDER=https://sso.datasektionen.se/op
SSO_URL=http://sso.nomad.dsekt.internal
OIDC_ID=karon
RUST_LOG: info
TZ: Europe/Stockholm
OIDC_REDIRECT_URL=https://karon.datasektionen.se/oidc/callback
HIVE_URL=https://hive.datasektionen.se/api/v1
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
