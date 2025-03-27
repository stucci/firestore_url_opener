# Firestore module configuration
terraform {
  required_providers {
    google-beta = {
      source  = "hashicorp/google-beta"
      version = "~> 5.0"
    }
  }
}

variable "project_id" {
  description = "The ID of the project in which the resource belongs"
  type        = string
}

# Enable Firebase API
resource "google_project_service" "firebase" {
  project = var.project_id
  service = "firebase.googleapis.com"

  disable_dependent_services = true
  disable_on_destroy        = false
}

# Create Firebase project
resource "google_firebase_project" "default" {
  provider = google-beta
  project  = var.project_id

  depends_on = [google_project_service.firebase]
}

# Enable Firestore API
resource "google_project_service" "firestore" {
  project = var.project_id
  service = "firestore.googleapis.com"

  disable_dependent_services = true
  disable_on_destroy        = false
}

# Create Firestore database
resource "google_firestore_database" "default" {
  project                     = var.project_id
  name                       = "(default)"
  location_id                = "us-central1"  # Iowa region
  type                       = "FIRESTORE_NATIVE"
  concurrency_mode           = "OPTIMISTIC"
  app_engine_integration_mode = "DISABLED"

  depends_on = [google_project_service.firestore]
}

resource "google_firestore_field" "expired_at" {
  project    = var.project_id
  database   = google_firestore_database.default.name
  collection = "shared_urls"
  field      = "expired_at"

  # enables a TTL policy for the document based on the value of entries with this field
  ttl_config {}
}

# Create Firestore security rules
resource "google_firebaserules_ruleset" "firestore" {
  project = var.project_id
  source {
    files {
      name    = "firestore.rules"
      content = file("${path.module}/firestore.rules")
    }
  }
}

# Release the security rules
resource "google_firebaserules_release" "firestore" {
  project      = var.project_id
  name         = "cloud.firestore"
  ruleset_name = google_firebaserules_ruleset.firestore.name
}

# Outputs
output "database_id" {
  description = "The ID of the Firestore database"
  value       = google_firestore_database.default.name
}

output "database_location" {
  description = "The location of the Firestore database"
  value       = google_firestore_database.default.location_id
} 
