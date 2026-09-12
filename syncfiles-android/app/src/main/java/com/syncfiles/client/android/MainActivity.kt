package com.syncfiles.client.android

import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.syncfiles.client.android.data.local.LocalFileSyncStore
import com.syncfiles.client.android.data.local.PendingSharesStore
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
import com.syncfiles.client.android.ui.share.SaveShareScreen
import com.syncfiles.client.android.ui.share.SaveShareViewModel
import com.syncfiles.client.android.ui.share.ShareInbox
import com.syncfiles.client.android.ui.theme.SyncFilesTheme

class MainActivity : ComponentActivity() {

    private val sessionStore: SessionStore by lazy { SessionStore(this) }
    private val localFileStore: LocalFileSyncStore by lazy { LocalFileSyncStore(this) }
    private val pendingSharesStore: PendingSharesStore by lazy { PendingSharesStore(this) }
    private val syncEngine: SyncEngine by lazy { (application as SyncFilesApplication).syncEngine }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        handleShareIntent(intent)
        setContent {
            SyncFilesTheme {
                SyncFilesApp(sessionStore, localFileStore)
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleShareIntent(intent)
    }

    /**
     * Un share de texto se junta en un único contenido y:
     * - con sesión+carpeta → pasa a la pantalla de guardado (in-flight);
     * - sin sesión o sin carpeta → cola SQLite de pendientes + Toast.
     */
    private fun handleShareIntent(intent: Intent?) {
        if (intent == null) return
        if (intent.action != Intent.ACTION_SEND && intent.action != Intent.ACTION_SEND_MULTIPLE) return

        val content = extractSharedTexts(intent).joinToString("\n")
        if (content.isBlank()) return

        val hasSession = sessionStore.getActiveSession() != null
        val hasRoot = sessionStore.getSyncRootUri() != null

        if (hasSession && hasRoot) {
            ShareInbox.pendingAutoNavDone = false
            ShareInbox.publish(content)
        } else {
            pendingSharesStore.insert(content)
            ShareInbox.pendingAutoNavDone = false
            Toast.makeText(
                this,
                "Guardado como pendiente — inicia sesión en SyncFiles para completarlo",
                Toast.LENGTH_LONG
            ).show()
        }
    }

    private fun extractSharedTexts(intent: Intent): List<String> {
        val result = mutableListOf<String>()
        if (intent.action == Intent.ACTION_SEND_MULTIPLE) {
            val texts = intent.getCharSequenceArrayListExtra(Intent.EXTRA_TEXT)
            if (!texts.isNullOrEmpty()) {
                texts.forEach { if (!it.isNullOrBlank()) result.add(it.toString()) }
                return result
            }
        }
        val extraText = intent.getStringExtra(Intent.EXTRA_TEXT)
        if (!extraText.isNullOrBlank()) result += extraText
        return result
    }
}

@Composable
fun SyncFilesApp(sessionStore: SessionStore, localFileStore: LocalFileSyncStore) {
    val navController = rememberNavController()
    val startDestination = if (sessionStore.getActiveSession() != null) "home" else "login"
    val appContext = LocalContext.current.applicationContext
    val syncEngine: SyncEngine = (appContext as SyncFilesApplication).syncEngine
    val pendingStore = remember { PendingSharesStore(appContext) }
    val inFlightShare by ShareInbox.inFlight.collectAsState()

    /** Navega a la pantalla de guardado si hay un share in-flight esperando. */
    fun navigateToSaveShare() {
        if (navController.currentDestination?.route == "save-share") return
        navController.navigate("save-share")
    }

    LaunchedEffect(inFlightShare) {
        if (inFlightShare != null && sessionStore.getActiveSession() != null) {
            navigateToSaveShare()
        }
    }

    NavHost(navController = navController, startDestination = startDestination) {
        composable("login") {
            val loginViewModel: LoginViewModel = viewModel(
                factory = LoginViewModel.Factory(sessionStore)
            )
            LoginScreen(
                onLoginSuccess = {
                    syncEngine.start()
                    ShareInbox.pendingAutoNavDone = false
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
            // Pendientes acumulados sin sesión/carpeta: al volver a Home
            // (login completado o carpeta recién elegida) se procesan.
            LaunchedEffect(Unit) {
                if (ShareInbox.pendingAutoNavDone) return@LaunchedEffect
                val hasRoot = sessionStore.getSyncRootUri() != null
                if (sessionStore.getActiveSession() != null && hasRoot && pendingStore.count() > 0) {
                    ShareInbox.pendingAutoNavDone = true
                    navigateToSaveShare()
                }
            }
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
                onBack = { navController.popBackStack() },
                onLoggedOut = {
                    navController.navigate("login") {
                        popUpTo(0) { inclusive = true }
                    }
                }
            )
        }
        composable("save-share") {
            val saveShareViewModel: SaveShareViewModel = viewModel(
                factory = SaveShareViewModel.Factory(sessionStore, syncEngine, pendingStore)
            )
            // Entrada sin in-flight: cargar el pendiente más antiguo (si no
            // hay nada, volver atrás).
            LaunchedEffect(Unit) {
                if (ShareInbox.inFlight.value == null &&
                    !saveShareViewModel.loadOldestPending()
                ) {
                    navController.popBackStack()
                }
            }
            // Un share in-flight (nuevo o re-compartido con la pantalla
            // abierta) reemplaza lo que hubiera en pantalla.
            LaunchedEffect(inFlightShare) {
                val inFlight = ShareInbox.inFlight.value ?: return@LaunchedEffect
                saveShareViewModel.loadInFlight(inFlight)
                ShareInbox.consume(inFlight)
            }
            SaveShareScreen(
                viewModel = saveShareViewModel,
                onBack = { navController.popBackStack() }
            )
        }
    }
}
