name := "tdh-protocol"
version := "0.1.0"
scalaVersion := "2.13.12"

// ScalaPB configuration
Compile / PB.targets := Seq(
  scalapb.gen() -> (Compile / sourceManaged).value / "scalapb"
)

// Disable serialization of options with default values
scalapbOptions := Seq(
  "flat_package"
)

libraryDependencies ++= Seq(
  "com.thesamet.scalapb" %% "scalapb-runtime" % scalapb.compiler.Version.scalapbVersion,
  "com.thesamet.scalapb" %% "scalapb-runtime-grpc" % scalapb.compiler.Version.scalapbVersion,
  // For later gRPC services
  "io.grpc" % "grpc-netty" % "1.60.0"
)

// GCP Artifact Registry publisher
// Read from env var PUBLISH_MAVEN_REGISTRY, fallback to default
publishTo := Some(
  "artifactregistry" at sys.env.getOrElse(
    "PUBLISH_MAVEN_REGISTRY",
    "https://asia-southeast1-maven.pkg.dev/tdg-dh-truehealth-core-nonprod/tdh-protocol-maven"
  )
)

// Authentication for GCP Artifact Registry
credentials += Credentials(
  "Artifact Registry",
  "oauth2.accesstoken",
  sys.env.getOrElse("ARTIFACT_REGISTRY_TOKEN", ""),
  ""
)
