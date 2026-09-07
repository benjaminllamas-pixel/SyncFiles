package com.syncfiles.client.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.compose.runtime.Composable
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.syncfiles.client.android.data.local.LocalFileSyncStore
import com.syncfiles.client.android.data.storage.SessionStore
import com.syncfiles.client.android.ui.home.HomeScreen
import com.syncfiles.client.android.ui.home.HomeViewModel
import com.syncfiles.client.android.ui.login.LoginScreen
import com.syncfiles.client.android.ui.login.LoginViewModel
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
    val appContext = androidx.compose.ui.platform.LocalContext.current.applicationContext

    NavHost(navController = navController, startDestination = startDestination) {
        composable("login") {
            val loginViewModel: LoginViewModel = viewModel(
                factory = LoginViewModel.Factory(sessionStore)
            )
            LoginScreen(
                onLoginSuccess = {
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
                    navController.navigate("login") {
                        popUpTo("home") { inclusive = true }
                    }
                }
            )
        }
    }
}
