// This file is only necessary so cdxgen doesn't fail the build
rootProject.name = "core-crypto"
// Include the sub-project by its directory path
include(":core-crypto-kotlin")
// Specify the location of the sub-project
project(":core-crypto-kotlin").projectDir = file("crypto-ffi/bindings")
