terraform {
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 5.0"
    }
  }
}

provider "google" {
  project = var.project_id
}

module "firestore" {
  source     = "./modules/firestore"
  project_id = var.project_id
} 
