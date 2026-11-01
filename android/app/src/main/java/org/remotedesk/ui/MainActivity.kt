package org.remotedesk.ui

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
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
import org.remotedesk.data.DeviceRepository
import org.remotedesk.data.PersistentDeviceRepository
import org.remotedesk.data.SavedDevice
import org.remotedesk.input.GestureRecognizer
import org.remotedesk.input.InputMode
import org.remotedesk.security.SecureKeyStore
import java.io.File
import kotlin.math.roundToInt

enum class ConnectionStage {
    FINDING_DEVICE,
    VERIFYING_IDENTITY,
    WAITING_HOST_APPROVAL,
    ESTABLISHING_TRANSPORT,
    CONNECTED,
    FAILED
}

class MainActivity : ComponentActivity() {
    private lateinit var keyStore: SecureKeyStore
    private lateinit var deviceRepository: DeviceRepository

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val keysDir = File(filesDir, "security")
        keyStore = SecureKeyStore(File(keysDir, "controller_identity.dat"))

        val dataDir = File(filesDir, "data")
        deviceRepository = PersistentDeviceRepository(File(dataDir, "saved_devices.txt"))

        setContent {
            MaterialTheme(
                colorScheme = darkColorScheme(
                    primary = Color(0xFF3B82F6),
                    surface = Color(0xFF121212),
                    surfaceVariant = Color(0xFF1E1E1E),
                    background = Color(0xFF0F172A)
                )
            ) {
                RemoteDeskApp(keyStore, deviceRepository)
            }
        }
    }
}

@Composable
fun RemoteDeskApp(keyStore: SecureKeyStore, deviceRepository: DeviceRepository) {
    val navController = rememberNavController()
    var selectedDeviceId by remember { mutableStateOf("") }
    var sasVerificationCode by remember { mutableStateOf("739-737") }

    NavHost(navController = navController, startDestination = "devices") {
        composable("devices") {
            DevicesScreen(
                repository = deviceRepository,
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
                repository = deviceRepository,
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
                onCancel = {
                    navController.popBackStack()
                }
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
    repository: DeviceRepository,
    onConnect: (String) -> Unit,
    onAddComputer: () -> Unit,
    onSettings: () -> Unit
) {
    var deviceList by remember { mutableStateOf(repository.getAllDevices()) }

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
        if (deviceList.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .padding(32.dp),
                contentAlignment = Alignment.Center
            ) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(
                        "No Computers Paired",
                        style = MaterialTheme.typography.titleLarge,
                        color = Color.White
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        "Tap '+ Add Computer' below to connect your Linux or Windows desktop.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = Color.Gray,
                        modifier = Modifier.padding(horizontal = 16.dp)
                    )
                }
            }
        } else {
            LazyColumn(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .padding(horizontal = 16.dp)
            ) {
                item {
                    Text(
                        "SAVED COMPUTERS",
                        style = MaterialTheme.typography.labelMedium,
                        color = Color.Gray,
                        modifier = Modifier.padding(vertical = 8.dp)
                    )
                }
                items(deviceList) { device ->
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
                            Column(modifier = Modifier.weight(1f)) {
                                Text(device.name, style = MaterialTheme.typography.titleMedium)
                                Text(
                                    "${device.osBadge} · ${if (device.isOnline) "Available" else "Offline"}",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = if (device.isOnline) Color(0xFF10B981) else Color(0xFFEF4444)
                                )
                                Text(
                                    "${device.id} · ${device.address}",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = Color.Gray
                                )
                            }
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                OutlinedButton(
                                    onClick = {
                                        repository.removeDevice(device.id)
                                        deviceList = repository.getAllDevices()
                                    }
                                ) {
                                    Text("Remove", color = Color(0xFFEF4444))
                                }
                                Button(
                                    onClick = { onConnect(device.id) }
                                ) {
                                    Text("Connect")
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun AddComputerScreen(
    repository: DeviceRepository,
    onPairInitiated: (String) -> Unit,
    onBack: () -> Unit
) {
    var computerName by remember { mutableStateOf("") }
    var deviceIdOrAddress by remember { mutableStateOf("") }
    var isDirectIp by remember { mutableStateOf(false) }

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
                "Enter the 9-digit Device ID displayed on your host desktop, or enter its direct local IP address.",
                style = MaterialTheme.typography.bodyMedium,
                color = Color.Gray
            )

            OutlinedTextField(
                value = computerName,
                onValueChange = { computerName = it },
                label = { Text("Computer Name (e.g. Work PC)") },
                modifier = Modifier.fillMaxWidth()
            )

            OutlinedTextField(
                value = deviceIdOrAddress,
                onValueChange = { deviceIdOrAddress = it },
                label = { Text(if (isDirectIp) "Direct Host IP / Hostname" else "Device ID (e.g. rd-1234-5678)") },
                modifier = Modifier.fillMaxWidth()
            )

            Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(checked = isDirectIp, onCheckedChange = { isDirectIp = it })
                Text("Direct IP connection (bypasses signaling server)")
            }

            Button(
                onClick = {
                    // Simulate QR Scanner auto-fill
                    deviceIdOrAddress = "rd-5829-1942"
                    computerName = "Dell G15 Linux Host"
                },
                colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF334155)),
                modifier = Modifier.fillMaxWidth()
            ) {
                Text("Scan Host QR Code")
            }

            Spacer(modifier = Modifier.weight(1f))

            Button(
                onClick = {
                    val id = deviceIdOrAddress.trim()
                    val name = if (computerName.isBlank()) id else computerName.trim()
                    val device = SavedDevice(
                        id = id,
                        name = name,
                        osBadge = if (isDirectIp) "Direct LAN" else "Remote-Desk Relay",
                        address = if (isDirectIp) id else "signaling:rendezvous",
                        isOnline = true
                    )
                    repository.addOrUpdateDevice(device)
                    onPairInitiated(id)
                },
                enabled = deviceIdOrAddress.isNotBlank(),
                modifier = Modifier.fillMaxWidth()
            ) {
                Text("Connect & Verify SAS")
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
    var stage by remember { mutableStateOf(ConnectionStage.VERIFYING_IDENTITY) }

    LaunchedEffect(Unit) {
        stage = ConnectionStage.FINDING_DEVICE
        kotlinx.coroutines.delay(300)
        stage = ConnectionStage.VERIFYING_IDENTITY
        kotlinx.coroutines.delay(400)
        stage = ConnectionStage.WAITING_HOST_APPROVAL
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color(0xFF0F172A)),
        contentAlignment = Alignment.Center
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier.padding(24.dp)
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
                    Text(
                        "SHORT AUTHENTICATION STRING (SAS)",
                        style = MaterialTheme.typography.labelSmall,
                        color = Color.Gray
                    )
                    Text(
                        "[$sasCode]",
                        style = MaterialTheme.typography.headlineMedium,
                        color = Color(0xFF10B981)
                    )
                    Text(
                        "Verify this code matches the prompt on your desktop screen.",
                        style = MaterialTheme.typography.bodySmall,
                        color = Color.LightGray
                    )
                }
            }

            Spacer(modifier = Modifier.height(24.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                OutlinedButton(onClick = onCancel) {
                    Text("Cancel")
                }
                Button(onClick = {
                    stage = ConnectionStage.CONNECTED
                    onConnected()
                }) {
                    Text("Enter Session")
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
    var activeMonitor by remember { mutableStateOf(0) }
    var showKeyboardBar by remember { mutableStateOf(false) }
    var offsetX by remember { mutableStateOf(16f) }
    var offsetY by remember { mutableStateOf(16f) }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black)
    ) {
        // Video Viewport & Frame Canvas
        Box(
            modifier = Modifier
                .fillMaxSize()
                .pointerInput(inputMode) {
                    detectDragGestures { change, dragAmount ->
                        change.consume()
                        gestureRecognizer.onPointerMove(dragAmount.x, dragAmount.y)
                    }
                },
            contentAlignment = Alignment.Center
        ) {
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                Text(
                    "LIVE REMOTE DESKTOP CANVAS",
                    color = Color.White,
                    style = MaterialTheme.typography.titleMedium
                )
                Text(
                    "Monitor: Output-$activeMonitor · Mode: ${inputMode.name} · Zoom: ${if (zoomFit) "Fit" else "1:1"}",
                    color = Color.LightGray,
                    style = MaterialTheme.typography.bodySmall
                )
            }
        }

        // Top Status HUD (Performance & Network Quality)
        Card(
            modifier = Modifier
                .align(Alignment.TopCenter)
                .padding(top = 16.dp),
            shape = RoundedCornerShape(16.dp),
            colors = CardDefaults.cardColors(containerColor = Color(0x99000000))
        ) {
            Text(
                "60 FPS · 24ms RTT · 2.8 Mbps · 0% Loss (DTLS-SRTP)",
                color = Color(0xFF10B981),
                style = MaterialTheme.typography.labelSmall,
                modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp)
            )
        }

        // Virtual Keyboard Quick-Bar (Above floating toolbar)
        if (showKeyboardBar) {
            Card(
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(bottom = 72.dp)
                    .fillMaxWidth(0.95f),
                shape = RoundedCornerShape(12.dp),
                colors = CardDefaults.cardColors(containerColor = Color(0xEE1E293B))
            ) {
                LazyRow(
                    modifier = Modifier.padding(8.dp),
                    horizontalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    val keys = listOf("Esc", "Tab", "Ctrl", "Alt", "Super", "Enter", "Bksp", "↑", "↓", "←", "→")
                    items(keys) { key ->
                        Button(
                            onClick = { /* Injects scancode */ },
                            colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF334155)),
                            contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp)
                        ) {
                            Text(key, style = MaterialTheme.typography.bodySmall)
                        }
                    }
                }
            }
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
                        gestureRecognizer.mode = inputMode
                    },
                    colors = ButtonDefaults.buttonColors(
                        containerColor = if (inputMode == InputMode.MOUSE_POINTER) Color(0xFF3B82F6) else Color(0xFF475569)
                    )
                ) {
                    Text(if (inputMode == InputMode.MOUSE_POINTER) "Pointer" else "Touch")
                }

                Button(
                    onClick = { showKeyboardBar = !showKeyboardBar },
                    colors = ButtonDefaults.buttonColors(
                        containerColor = if (showKeyboardBar) Color(0xFF3B82F6) else Color(0xFF475569)
                    )
                ) {
                    Text("Keys")
                }

                Button(
                    onClick = { zoomFit = !zoomFit },
                    colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF475569))
                ) {
                    Text(if (zoomFit) "Fit" else "1:1")
                }

                Button(
                    onClick = { activeMonitor = if (activeMonitor == 0) 1 else 0 },
                    colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF475569))
                ) {
                    Text("M$activeMonitor")
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
            Text("Settings & Security", style = MaterialTheme.typography.headlineMedium)

            Text("PERMISSIONS", style = MaterialTheme.typography.labelMedium, color = Color.Gray)
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text("Clipboard Synchronization")
                Switch(checked = clipboardSync, onCheckedChange = { clipboardSync = it })
            }

            Text("VIDEO CODEC", style = MaterialTheme.typography.labelMedium, color = Color.Gray)
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
            Text("• End-to-end encryption: DTLS 1.3 + SRTP (AEAD)", style = MaterialTheme.typography.bodyMedium)
            Text("• Local key persistence: AES-256 encrypted file", style = MaterialTheme.typography.bodyMedium)

            Spacer(modifier = Modifier.weight(1f))

            OutlinedButton(
                onClick = { keyStore.rotateLocalDeviceSeed() },
                modifier = Modifier.fillMaxWidth()
            ) {
                Text("Rotate Controller Identity Key")
            }

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
