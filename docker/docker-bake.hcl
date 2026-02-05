# ═══════════════════════════════════════════════════════════════════════════
# Monmouth Docker Buildx Bake Configuration
# ═══════════════════════════════════════════════════════════════════════════

variable "REGISTRY" {
  default = "ghcr.io/MonmouthFND/monmouth-node"
}

variable "PLATFORMS" {
  default = "linux/amd64,linux/arm64"
}

variable "GIT_REF_NAME" {
  default = "main"
}

variable "BUILD_PROFILE" {
  default = "release"
}

# ───────────────────────────────────────────────────────────────────────────
# Groups
# ───────────────────────────────────────────────────────────────────────────

group "default" {
  targets = ["monmouth"]
}

group "all" {
  targets = ["monmouth", "monmouth-dev"]
}

# ───────────────────────────────────────────────────────────────────────────
# Base target for shared configuration
# ───────────────────────────────────────────────────────────────────────────

target "docker-metadata-action" {
  tags = ["${REGISTRY}/monmouth:${GIT_REF_NAME}"]
}

# ───────────────────────────────────────────────────────────────────────────
# Production build - multi-platform
# ───────────────────────────────────────────────────────────────────────────

target "monmouth" {
  inherits   = ["docker-metadata-action"]
  context    = ".."
  dockerfile = "docker/Dockerfile"
  platforms  = split(",", PLATFORMS)
  args = {
    BUILD_PROFILE = BUILD_PROFILE
  }
}

# ───────────────────────────────────────────────────────────────────────────
# Local development build - single platform, local only
# ───────────────────────────────────────────────────────────────────────────

target "monmouth-local" {
  context    = ".."
  dockerfile = "docker/Dockerfile"
  platforms  = ["linux/amd64"]
  tags       = ["monmouth:local"]
  args = {
    BUILD_PROFILE = "release"
  }
}

# ───────────────────────────────────────────────────────────────────────────
# Development build with debug symbols
# ───────────────────────────────────────────────────────────────────────────

target "monmouth-dev" {
  context    = ".."
  dockerfile = "docker/Dockerfile"
  platforms  = ["linux/amd64"]
  tags       = ["monmouth:dev"]
  args = {
    BUILD_PROFILE = "dev"
  }
}
