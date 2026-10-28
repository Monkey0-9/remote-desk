package org.remotedesk.ui

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import org.remotedesk.input.InputMode
import kotlin.math.roundToInt

data class SavedDevice(
    val id: String,
    val name: String,
    val osBadge: String,
    val address: String,
    val isOnline: Boolean
)

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme(
                colorScheme = darkColorScheme(
                    primary = Color(0xFF3B82F6),
                    surface = Color(0xFF121212),
                    surfaceVariant = Color(0xFF1E1E1E),
                    background = Color(0xFF0F172A)
                )
            ) {
                RemoteDeskApp()
            }
        }
    }
}

@Composable
fun RemoteDeskApp() {
    val navController = rememberNavController()

    NavHost(navController = navController, startDestination = "devices") {
        composable("devices") {
            DevicesScreen(
                onConnect = { navController.navigate("session") },
                onSettings = { navController.navigate("settings") }
            )
        }
        composable("session") {
            SessionCanvasScreen(
                onDisconnect = { navController.popBackStack() }
            )
        }
        composable("settings") {
            SettingsScreen(onBack = { navController.popBackStack() })
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DevicesScreen(onConnect: (String) -> Unit, onSettings: () -> Unit) {
    val devices = remember {
        mutableStateListOf(
            SavedDevice("rd-3818-9087", "Linux Desktop", "Linux (Wayland)", "192.168.1.120", true),
            SavedDevice("rd-8402-1923", "Work Laptop", "Windows 11", "10.0.0.45", false)
        )
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Remote-Desk") },
                actions = {
                    TextButton(onClick = onSettings) {
                        Text("Settings", color = MaterialTheme.colorScheme.primary)
                    }
                }
            )
        },
        floatingActionButton = {
            FloatingActionButton(onClick = { /* Add computer flow */ }) {
                Text("+", style = MaterialTheme.typography.titleLarge)
            }
        }
    ) { padding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(horizontal = 16.dp)
        ) {
            item {
                Text(
                    "YOUR COMPUTERS",
                    style = MaterialTheme.typography.labelMedium,
                    color = Color.Gray,
                    modifier = Modifier.padding(vertical = 8.dp)
                )
            }
            items(devices) { device ->
                Card(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 6.dp),
                    shape = RoundedCornerShape(12.dp),
                    colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant)
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(16.dp),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column {
                            Text(device.name, style = MaterialTheme.typography.titleMedium)
                            Text(
                                "${device.osBadge} · ${if (device.isOnline) "Available" else "Offline"}",
                                style = MaterialTheme.typography.bodySmall,
                                color = if (device.isOnline) Color(0xFF10B981) else Color(0xFFEF4444)
                            )
                        }
                        Button(
                            onClick = { onConnect(device.id) },
                            enabled = device.isOnline
                        ) {
                            Text("Connect")
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun SessionCanvasScreen(onDisconnect: () -> Unit) {
    var inputMode by remember { mutableStateOf(InputMode.DIRECT_TOUCH) }
    var offsetX by remember { mutableStateOf(16f) }
    var offsetY by remember { mutableStateOf(16f) }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black)
    ) {
        // Live Screen Canvas Viewport
        Box(
            modifier = Modifier.fillMaxSize(),
            contentAlignment = Alignment.Center
        ) {
            Text(
                "LIVE REMOTE DESKTOP CANVAS\n(Wayland ScreenCast / 60 FPS)",
                color = Color.White,
                style = MaterialTheme.typography.titleMedium
            )
        }

        // Floating Draggable Pill Toolbar
        Card(
            modifier = Modifier
                .align(Alignment.BottomEnd)
                .offset { IntOffset(-offsetX.roundToInt(), -offsetY.roundToInt()) }
                .pointerInput(Unit) {
                    detectDragGestures { change, dragAmount ->
                        change.consume()
                        offsetX -= dragAmount.x
                        offsetY -= dragAmount.y
                    }
                },
            shape = RoundedCornerShape(24.dp),
            colors = CardDefaults.cardColors(containerColor = Color(0xCC1E293B))
        ) {
            Row(
                modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Button(
                    onClick = {
                        inputMode = if (inputMode == InputMode.DIRECT_TOUCH) {
                            InputMode.MOUSE_POINTER
                        } else {
                            InputMode.DIRECT_TOUCH
                        }
                    },
                    colors = ButtonDefaults.buttonColors(
                        containerColor = if (inputMode == InputMode.MOUSE_POINTER) Color(0xFF3B82F6) else Color(0xFF475569)
                    )
                ) {
                    Text(if (inputMode == InputMode.MOUSE_POINTER) "Pointer" else "Touch")
                }

                Button(
                    onClick = onDisconnect,
                    colors = ButtonDefaults.buttonColors(containerColor = Color(0xFFEF4444))
                ) {
                    Text("End")
                }
            }
        }
    }
}

@Composable
fun SettingsScreen(onBack: () -> Unit) {
    Scaffold(
        topBar = {
            TextButton(onClick = onBack, modifier = Modifier.padding(8.dp)) {
                Text("< Back", color = MaterialTheme.colorScheme.primary)
            }
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp)
        ) {
            Text("Settings & Privacy", style = MaterialTheme.typography.titleLarge)
            Spacer(modifier = Modifier.height(16.dp))
            Text("No Screen Recording by Default: Enforced", style = MaterialTheme.typography.bodyMedium)
            Text("End-to-End Encryption: Active (DTLS-SRTP)", style = MaterialTheme.typography.bodyMedium)
            Text("Local Keystore: Hardware Protected", style = MaterialTheme.typography.bodyMedium)
        }
    }
}
