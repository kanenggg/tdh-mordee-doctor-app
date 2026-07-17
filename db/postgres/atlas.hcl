variable "database_url" {
  type    = string
  default = getenv("DATABASE_URL")
}

variable "atlas_dev_url" {
  type    = string
  default = getenv("ATLAS_DEV_URL")
}

variable "schema_src" {
  type    = string
  default = "file://schema"
}

variable "migration_dir" {
  type    = string
  default = "file://migrations"
}

env "local-container" {
  src = var.schema_src
  url = var.database_url
  dev = var.atlas_dev_url
  migrate = {
    dir    = var.migration_dir
    format = atlas
  }
}

env "local" {
  src = var.schema_src
  url = var.database_url
  dev = var.atlas_dev_url
  migrate = {
    dir    = var.migration_dir
    format = atlas
  }
}

env "tdh-k8s" {
  src = var.schema_src
  url = var.database_url
  dev = var.atlas_dev_url
  migrate = {
    dir    = var.migration_dir
    format = atlas
  }
}
