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
import org.remotedesk.input.GestureRecognizer
import org.remotedesk.input.InputMode
import org.remotedesk.security.SecureKeyStore
import kotlin.math.roundToInt

data class SavedDevice(
    val id: String,
    val name: String,
    val osBadge: String,
    val address: String,
    val isOnline: Boolean
)

enum class ConnectionStage {
    FINDING_DEVICE,
    VERIFYING_IDENTITY,
    WAITING_HOST_APPROVAL,
    NEGOTIATING_MEDIA,
    CONNECTED,
    FAILED
}

class MainActivity : ComponentActivity() {
    private val keyStore = SecureKeyStore()

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
                RemoteDeskApp(keyStore)
            }
        }
    }
}

@Composable
fun RemoteDeskApp(keyStore: SecureKeyStore) {
    val navController = rememberNavController()
    var selectedDeviceId by remember { mutableStateOf("") }
    var sasVerificationCode by remember { mutableStateOf("739-737") }

    NavHost(navController = navController, startDestination = "devices") {
        composable("devices") {
            DevicesScreen(
                onConnect = { id ->
                    selectedDeviceId = id
                    navController.navigate("connecting")
                },
                onAddComputer = { navController.navigate("add_computer") },
                onSettings = { navController.navigate("settings") }
            )
        }
        composable("add_computer") {
            AddComputerScreen(
                onPairInitiated = { id ->
                    selectedDeviceId = id
                    navController.navigate("connecting")
                },
                onBack = { navController.popBackStack() }
            )
        }
        composable("connecting") {
            ConnectionProgressScreen(
                deviceId = selectedDeviceId,
                sasCode = sasVerificationCode,
                onConnected = {
                    navController.navigate("session") {
                        popUpTo("devices")
                    }
                },
                onCancel = { navController.popBackStack() }
            )
        }
        composable("session") {
            SessionCanvasScreen(
                onDisconnect = {
                    navController.navigate("devices") {
                        popUpTo("devices") { inclusive = true }
                    }
                }
            )
        }
        composable("settings") {
            SettingsScreen(
                keyStore = keyStore,
                onBack = { navController.popBackStack() }
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DevicesScreen(
    onConnect: (String) -> Unit,
    onAddComputer: () -> Unit,
    onSettings: () -> Unit
) {
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
            ExtendedFloatingActionButton(
                onClick = onAddComputer,
                containerColor = MaterialTheme.colorScheme.primary
            ) {
                Text("+ Add Computer")
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
                            Text(device.id, style = MaterialTheme.typography.bodySmall, color = Color.Gray)
                        }
                        Button(
                            onClick = { onConnect(device.id) },
                            enabled = device.isOnline
                        ) {
                            Text(if (device.isOnline) "Connect" else "Retry")
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun AddComputerScreen(onPairInitiated: (String) -> Unit, onBack: () -> Unit) {
    var computerName by remember { mutableStateOf("") }
    var deviceIdOrAddress by remember { mutableStateOf("") }

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
                .padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text("Add Computer", style = MaterialTheme.typography.headlineMedium)
            Text(
                "A Device ID is a locator, not an authentication credential. You will verify a 6-digit SAS code on the host.",
                style = MaterialTheme.typography.bodySmall,
                color = Color.Gray
            )

            OutlinedTextField(
                value = computerName,
                onValueChange = { computerName = it },
                label = { Text("Computer Name (Alias)") },
                modifier = Modifier.fillMaxWidth()
            )

            OutlinedTextField(
                value = deviceIdOrAddress,
                onValueChange = { deviceIdOrAddress = it },
                label = { Text("Device ID (rd-XXXX-XXXX) or IP") },
                modifier = Modifier.fillMaxWidth()
            )

            Button(
                onClick = { onPairInitiated(deviceIdOrAddress.ifBlank { "rd-9999-0000" }) },
                enabled = deviceIdOrAddress.isNotBlank(),
                modifier = Modifier.fillMaxWidth()
            ) {
                Text("Start Pairing Handshake")
            }
        }
    }
}

@Composable
fun ConnectionProgressScreen(
    deviceId: String,
    sasCode: String,
    onConnected: () -> Unit,
    onCancel: () -> Unit
) {
    var stage by remember { mutableStateOf(ConnectionStage.WAITING_HOST_APPROVAL) }

    Scaffold { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(32.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center
        ) {
            CircularProgressIndicator(color = MaterialTheme.colorScheme.primary)
            Spacer(modifier = Modifier.height(24.dp))

            Text("Connecting to $deviceId...", style = MaterialTheme.typography.titleLarge)
            Spacer(modifier = Modifier.height(8.dp))

            Text("Stage: ${stage.name.replace('_', ' ')}", color = Color(0xFF3B82F6))
            Spacer(modifier = Modifier.height(16.dp))

            Card(
                shape = RoundedCornerShape(12.dp),
                colors = CardDefaults.cardColors(containerColor = Color(0xFF1E293B)),
                modifier = Modifier.padding(16.dp)
            ) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    horizontalAlignment = Alignment.CenterHorizontally
                ) {
                    Text("SHORT AUTHENTICATION STRING (SAS)", style = MaterialTheme.typography.labelSmall, color = Color.Gray)
                    Text("[$sasCode]", style = MaterialTheme.typography.headlineMedium, color = Color(0xFF10B981))
                    Text("Verify this code matches the host desktop prompt.", style = MaterialTheme.typography.bodySmall, color = Color.LightGray)
                }
            }

            Spacer(modifier = Modifier.height(24.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                OutlinedButton(onClick = onCancel) {
                    Text("Cancel")
                }
                Button(onClick = onConnected) {
                    Text("Simulate Approve & Stream")
                }
            }
        }
    }
}

@Composable
fun SessionCanvasScreen(onDisconnect: () -> Unit) {
    val gestureRecognizer = remember { GestureRecognizer() }
    var inputMode by remember { mutableStateOf(InputMode.DIRECT_TOUCH) }
    var zoomFit by remember { mutableStateOf(true) }
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
                "LIVE REMOTE DESKTOP CANVAS\n(Wayland / PipeWire DMA-BUF Stream)\nMode: ${inputMode.name} · Zoom: ${if (zoomFit) "Fit" else "100%"}",
                color = Color.White,
                style = MaterialTheme.typography.titleMedium
            )
        }

        // Floating Draggable Pill Toolbar (Collapsible / Unobtrusive)
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
                        gestureRecognizer.mode = inputMode
                    },
                    colors = ButtonDefaults.buttonColors(
                        containerColor = if (inputMode == InputMode.MOUSE_POINTER) Color(0xFF3B82F6) else Color(0xFF475569)
                    )
                ) {
                    Text(if (inputMode == InputMode.MOUSE_POINTER) "Pointer" else "Touch")
                }

                Button(
                    onClick = { zoomFit = !zoomFit },
                    colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF475569))
                ) {
                    Text(if (zoomFit) "Fit" else "1:1")
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
fun SettingsScreen(keyStore: SecureKeyStore, onBack: () -> Unit) {
    var clipboardSync by remember { mutableStateOf(false) }
    var preferAv1 by remember { mutableStateOf(false) }

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
                .padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Text("Settings & Privacy", style = MaterialTheme.typography.headlineMedium)

            Text("PRIVACY & PERMISSIONS", style = MaterialTheme.typography.labelMedium, color = Color.Gray)
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text("Clipboard Synchronization")
                Switch(checked = clipboardSync, onCheckedChange = { clipboardSync = it })
            }

            Text("VIDEO & CODEC", style = MaterialTheme.typography.labelMedium, color = Color.Gray)
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text("Prefer AV1 over H.264")
                Switch(checked = preferAv1, onCheckedChange = { preferAv1 = it })
            }

            Spacer(modifier = Modifier.height(8.dp))
            Text("SECURITY GUARANTEES", style = MaterialTheme.typography.labelMedium, color = Color.Gray)
            Text("• Zero screen recording by default: ENFORCED", style = MaterialTheme.typography.bodyMedium)
            Text("• End-to-end encryption: DTLS 1.3 + SRTP", style = MaterialTheme.typography.bodyMedium)
            Text("• Keystore: Hardware backed (OWASP MASVS-STORAGE-1)", style = MaterialTheme.typography.bodyMedium)

            Spacer(modifier = Modifier.weight(1f))
            Button(
                onClick = { keyStore.clearAllKeys() },
                colors = ButtonDefaults.buttonColors(containerColor = Color(0xFFEF4444)),
                modifier = Modifier.fillMaxWidth()
            ) {
                Text("Revoke All Paired Device Keys")
            }
        }
    }
}
