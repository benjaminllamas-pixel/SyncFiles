plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.syncfiles.client.android"
    compileSdk = 34

    // Firma release opcional: definir en ~/.gradle/gradle.properties o en
    // syncfiles-android/gradle.properties.local (no versionado):
    //   SF_KEYSTORE_FILE, SF_KEYSTORE_PASSWORD, SF_KEYSTORE_ALIAS, SF_KEY_PASSWORD
    // Sin esas propiedades, assembleRelease genera un APK sin firmar.
    val keystoreFile = project.findProperty("SF_KEYSTORE_FILE") as String?
    val keystorePassword = project.findProperty("SF_KEYSTORE_PASSWORD") as String?
    val keyAliasProp = project.findProperty("SF_KEYSTORE_ALIAS") as String?
    val keyPasswordProp = project.findProperty("SF_KEY_PASSWORD") as String?
    val hasSigningProps = !keystoreFile.isNullOrBlank() &&
        !keystorePassword.isNullOrBlank() &&
        !keyAliasProp.isNullOrBlank() &&
        !keyPasswordProp.isNullOrBlank()

    signingConfigs {
        if (hasSigningProps) {
            create("releaseConfig") {
                storeFile = rootProject.file(keystoreFile!!)
                storePassword = keystorePassword
                keyAlias = keyAliasProp
                keyPassword = keyPasswordProp
            }
        }
    }

    defaultConfig {
        applicationId = "com.syncfiles.client.android"
        minSdk = 29
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            if (hasSigningProps) {
                signingConfig = signingConfigs.getByName("releaseConfig")
            }
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
    }

    composeOptions {
        kotlinCompilerExtensionVersion = "1.5.14"
    }

    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2024.06.00")
    implementation(composeBom)

    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    debugImplementation("androidx.compose.ui:ui-tooling")

    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.documentfile:documentfile:1.1.0")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.3")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.3")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.3")
    implementation("androidx.activity:activity-compose:1.9.0")
    implementation("androidx.navigation:navigation-compose:2.7.7")

    implementation("com.squareup.retrofit2:retrofit:2.11.0")
    implementation("com.squareup.retrofit2:converter-gson:2.11.0")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("com.squareup.okhttp3:logging-interceptor:4.12.0")

    implementation("androidx.security:security-crypto:1.1.0-alpha06")
    implementation("androidx.datastore:datastore-preferences:1.1.1")

    implementation("androidx.work:work-runtime-ktx:2.9.0")

    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.8.1")

    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.6.1")
}
