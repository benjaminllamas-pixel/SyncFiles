package com.syncfiles.client.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.syncfiles.client.android.data.local.LocalFileSyncStore
import com.syncfiles.client.android.data.local.SyncEngine
import com.syncfiles.client.android.data.storage.SessionStore
import com.syncfiles.client.android.ui.conflicts.ConflictsScreen
import com.syncfiles.client.android.ui.conflicts.ConflictsViewModel
import com.syncfiles.client.android.ui.files.FilesScreen
import com.syncfiles.client.android.ui.files.FilesViewModel
import com.syncfiles.client.android.ui.home.HomeScreen
import com.syncfiles.client.android.ui.home.HomeViewModel
import com.syncfiles.client.android.ui.login.LoginScreen
import com.syncfiles.client.android.ui.login.LoginViewModel
import com.syncfiles.client.android.ui.settings.SettingsScreen
import com.syncfiles.client.android.ui.settings.SettingsViewModel
import com.syncfiles.client.android.ui.theme.SyncFilesTheme

class MainActivity : ComponentActivity() {

    private val sessionStore: SessionStore by lazy { SessionStore(this) }
    private val localFileStore: LocalFileSyncStore by lazy { LocalFileSyncStore(this) }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            SyncFilesTheme {
                SyncFilesApp(sessionStore, localFileStore)
            }
        }
    }
}

@Composable
fun SyncFilesApp(sessionStore: SessionStore, localFileStore: LocalFileSyncStore) {
    val navController = rememberNavController()
    val startDestination = if (sessionStore.getActiveSession() != null) "home" else "login"
    val appContext = LocalContext.current.applicationContext
    val syncEngine: SyncEngine = (appContext as SyncFilesApplication).syncEngine

    NavHost(navController = navController, startDestination = startDestination) {
        composable("login") {
            val loginViewModel: LoginViewModel = viewModel(
                factory = LoginViewModel.Factory(sessionStore)
            )
            LoginScreen(
                onLoginSuccess = {
                    syncEngine.start()
                    navController.navigate("home") {
                        popUpTo("login") { inclusive = true }
                    }
                },
                viewModel = loginViewModel
            )
        }
        composable("home") {
            val homeViewModel: HomeViewModel = viewModel(
                factory = HomeViewModel.Factory(appContext, sessionStore, localFileStore)
            )
            HomeScreen(
                viewModel = homeViewModel,
                onSessionExpired = {
                    syncEngine.stop()
                    navController.navigate("login") {
                        popUpTo("home") { inclusive = true }
                    }
                },
                onOpenFiles = { navController.navigate("files") },
                onOpenConflicts = { navController.navigate("conflicts") },
                onOpenSettings = { navController.navigate("settings") }
            )
        }
        composable("files") {
            val filesViewModel: FilesViewModel = viewModel(
                factory = FilesViewModel.Factory(appContext, sessionStore)
            )
            FilesScreen(
                viewModel = filesViewModel,
                onBack = { navController.popBackStack() }
            )
        }
        composable("conflicts") {
            val conflictsViewModel: ConflictsViewModel = viewModel(
                factory = ConflictsViewModel.Factory(appContext, sessionStore)
            )
            ConflictsScreen(
                viewModel = conflictsViewModel,
                onBack = { navController.popBackStack() }
            )
        }
        composable("settings") {
            val settingsViewModel: SettingsViewModel = viewModel(
                factory = SettingsViewModel.Factory(appContext, sessionStore, syncEngine)
            )
            SettingsScreen(
                viewModel = settingsViewModel,
                onLoggedOut = {
                    navController.navigate("login") {
                        popUpTo(0) { inclusive = true }
                    }
                }
            )
        }
    }
}
