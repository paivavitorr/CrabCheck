package com.crabcheck.alarmlib

import android.Manifest
import android.annotation.SuppressLint
import android.app.KeyguardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.Matrix
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.os.Bundle
import android.os.CountDownTimer
import android.text.Editable
import android.text.TextWatcher
import android.util.Size
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.label.ImageLabeler
import com.google.mlkit.vision.label.ImageLabeling
import com.google.mlkit.vision.label.defaults.ImageLabelerOptions
import uniffi.crabcheck_core_ffi.FfiAlarmDto
import uniffi.crabcheck_core_ffi.ffiGetAlarm
import uniffi.crabcheck_core_ffi.ffiRecordOutcome
import uniffi.crabcheck_core_ffi.ffiRecordOutcomeWithProof
import uniffi.crabcheck_core_ffi.ffiRescheduleAfterTrigger
import java.io.File
import java.io.FileOutputStream
import java.util.Locale
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

class LockActivity : AppCompatActivity() {

    companion object {
        private const val REQUEST_CAMERA_PERMISSION = 1001
        private const val REQUIRED_FRAMES = 5
        private const val CONFIDENCE_THRESHOLD = 0.70f
        private const val SNOOZE_DURATION_MS = 5L * 60L * 1000L
        private const val PANIC_COUNTDOWN_MS = 3L * 60L * 1000L
        private const val COMMITMENT_PHRASE = "ESTOU DESISTINDO DO MEU COMPROMISSO"
    }

    private var cameraExecutor: ExecutorService? = null
    private var cameraProvider: ProcessCameraProvider? = null
    private var imageLabeler: ImageLabeler? = null
    private val hasCompletedProof = AtomicBoolean(false)
    private var panicDialog: AlertDialog? = null
    private var panicTimer: CountDownTimer? = null

    private var currentDbPath: String = ""
    private var currentAlarmId: String = "spike-001"
    private var currentTargetLabel: String = "Book"
    private var previewView: PreviewView? = null
    private var telemetryView: TextView? = null
    private var retryPermissionBtn: Button? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        cameraExecutor = Executors.newSingleThreadExecutor()
        enableShowWhenLocked()
        renderAlarmFromIntent(intent)
    }

    override fun onNewIntent(intent: Intent?) {
        super.onNewIntent(intent)
        if (intent != null) {
            val incomingAlarmId = intent.getStringExtra(AlarmScheduler.EXTRA_ALARM_ID)
            if (incomingAlarmId != null && incomingAlarmId != currentAlarmId) {
                setIntent(intent)
                renderAlarmFromIntent(intent)
            }
        }
    }

    override fun onDestroy() {
        if (isChangingConfigurations) {
            dismissPanicDialog()
            stopCameraSession()
            cameraExecutor?.shutdown()
            cameraExecutor = null
            super.onDestroy()
            return
        }

        dismissPanicDialog()
        stopCameraSession()
        stopAlarmAudioService()
        AlarmStateManager.onAlarmDismissed(currentAlarmId)
        cameraExecutor?.shutdown()
        cameraExecutor = null
        super.onDestroy()
    }

    private fun stopAlarmAudioService() {
        try {
            stopService(Intent(this, AlarmAudioService::class.java))
        } catch (_: Throwable) {
        }
    }

    private fun dismissPanicDialog() {
        try {
            panicTimer?.cancel()
            panicTimer = null
            panicDialog?.dismiss()
            panicDialog = null
            AlarmStateManager.setPanicActive(false)
        } catch (_: Throwable) {
        }
    }

    private fun stopCameraSession() {
        hasCompletedProof.set(true)
        try {
            cameraProvider?.unbindAll()
        } catch (_: Throwable) {
        }
        try {
            imageLabeler?.close()
        } catch (_: Throwable) {
        }
        imageLabeler = null
    }

    private fun createTactileButton(
        label: String,
        bgHex: String,
        textHex: String = "#FFFFFF",
        strokeHex: String = "#334155"
    ): Button {
        val bg = GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            cornerRadius = 24f
            setColor(Color.parseColor(bgHex))
            setStroke(4, Color.parseColor(strokeHex))
        }
        return Button(this).apply {
            text = label
            setTextColor(Color.parseColor(textHex))
            textSize = 15f
            typeface = Typeface.DEFAULT_BOLD
            isAllCaps = false
            background = bg
            setPadding(36, 32, 36, 32)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                setMargins(0, 14, 0, 0)
            }
        }
    }

    private fun renderAlarmFromIntent(currentIntent: Intent) {
        stopCameraSession()

        val alarmId = currentIntent.getStringExtra(AlarmScheduler.EXTRA_ALARM_ID) ?: "spike-001"
        val dbPath = AlarmScheduler.dbPath(this)
        currentAlarmId = alarmId
        currentDbPath = dbPath

        val dto: FfiAlarmDto? = try {
            ffiGetAlarm(dbPath, alarmId)
        } catch (_: Throwable) {
            null
        }

        val level = dto?.level ?: 3
        val targetLabel = dto?.targetLabel?.ifBlank { "Book" } ?: "Book"
        currentTargetLabel = targetLabel

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            setBackgroundColor(Color.parseColor("#0F172A"))
            setPadding(48, 64, 48, 56)
        }

        val badgeColor = when (level) {
            3 -> "#E03E2D"
            2 -> "#D97706"
            else -> "#475569"
        }
        val badgeLabel = when (level) {
            3 -> "🦀 PACTO NÍVEL 3 • PROVA VISUAL IA"
            2 -> "⚡ PACTO NÍVEL 2 • PERSISTENTE"
            else -> "🔔 PACTO NÍVEL 1 • LEMBRETE ATIVO"
        }

        val badgeView = TextView(this).apply {
            text = badgeLabel
            textSize = 12f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.WHITE)
            gravity = Gravity.CENTER
            background = GradientDrawable().apply {
                shape = GradientDrawable.RECTANGLE
                cornerRadius = 999f
                setColor(Color.parseColor(badgeColor))
            }
            setPadding(32, 14, 32, 14)
        }
        root.addView(badgeView)

        val timeText = if (dto != null) {
            String.format(Locale.US, "%02d:%02d", dto.hour.toInt(), dto.minute.toInt())
        } else {
            "AGORA"
        }

        val clockView = TextView(this).apply {
            text = timeText
            textSize = 46f
            typeface = Typeface.MONOSPACE
            setTextColor(Color.parseColor("#F8FAFC"))
            gravity = Gravity.CENTER
            setPadding(0, 20, 0, 4)
        }
        root.addView(clockView)

        val titleView = TextView(this).apply {
            text = dto?.title ?: "Compromisso #$alarmId"
            textSize = 24f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.WHITE)
            gravity = Gravity.CENTER
            setPadding(0, 0, 0, 24)
        }
        root.addView(titleView)

        if (level == 3) {
            renderLevel3ProofUi(root, dbPath, alarmId, targetLabel)
        } else {
            renderStandardLevelUi(root, dbPath, alarmId, level)
        }

        setContentView(root)

        if (level == 3) {
            ensureCameraPermissionAndStart()
        }
    }

    private fun renderLevel3ProofUi(
        root: LinearLayout,
        dbPath: String,
        alarmId: String,
        targetLabel: String
    ) {
        val instructionView = TextView(this).apply {
            text = "📷 Aponte a câmara para: $targetLabel\n(Validação contínua: $REQUIRED_FRAMES frames ≥70%)"
            textSize = 14f
            setTextColor(Color.parseColor("#FCA5A5"))
            typeface = Typeface.DEFAULT_BOLD
            gravity = Gravity.CENTER
            background = GradientDrawable().apply {
                shape = GradientDrawable.RECTANGLE
                cornerRadius = 20f
                setColor(Color.parseColor("#1E293B"))
                setStroke(3, Color.parseColor("#E03E2D"))
            }
            setPadding(28, 22, 28, 22)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                setMargins(0, 0, 0, 20)
            }
        }
        root.addView(instructionView)

        val cameraPreview = PreviewView(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                0,
                1f
            )
        }
        previewView = cameraPreview
        root.addView(cameraPreview)

        val statusView = TextView(this).apply {
            text = "Progresso: 0/$REQUIRED_FRAMES frames | Detetado: —"
            textSize = 13f
            typeface = Typeface.MONOSPACE
            setTextColor(Color.parseColor("#6EE7B7"))
            gravity = Gravity.CENTER
            background = GradientDrawable().apply {
                shape = GradientDrawable.RECTANGLE
                cornerRadius = 16f
                setColor(Color.parseColor("#064E3B"))
            }
            setPadding(24, 18, 24, 18)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                setMargins(0, 20, 0, 8)
            }
        }
        telemetryView = statusView
        root.addView(statusView)

        val permBtn = createTactileButton(
            "Conceder Permissão de Câmara",
            "#0D9488",
            "#FFFFFF",
            "#10B981"
        ).apply {
            visibility = View.GONE
            setOnClickListener {
                ActivityCompat.requestPermissions(
                    this@LockActivity,
                    arrayOf(Manifest.permission.CAMERA),
                    REQUEST_CAMERA_PERMISSION
                )
            }
        }
        retryPermissionBtn = permBtn
        root.addView(permBtn)

        val panicBtn = createTactileButton(
            "Saída de Emergência (3m + Compromisso)",
            "#1E293B",
            "#FCA5A5",
            "#7F1D1D"
        ).apply {
            setOnClickListener {
                showStructuredPanicDialog(dbPath, alarmId)
            }
        }
        root.addView(panicBtn)
    }

    private fun renderStandardLevelUi(
        root: LinearLayout,
        dbPath: String,
        alarmId: String,
        level: Int
    ) {
        val spacer = View(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                40
            )
        }
        root.addView(spacer)

        val completeBtn = createTactileButton(
            "✓ Confirmar Cumprimento (CUMPRIDO)",
            "#059669",
            "#FFFFFF",
            "#10B981"
        ).apply {
            setOnClickListener {
                recordAndClose(dbPath, alarmId, "CUMPRIDO")
            }
        }
        root.addView(completeBtn)

        if (level == 2) {
            val snoozeBtn = createTactileButton(
                "⏳ Adiar 5 Minutos (ADIADO)",
                "#D97706",
                "#FFFFFF",
                "#F59E0B"
            ).apply {
                setOnClickListener {
                    snoozeAndClose(dbPath, alarmId)
                }
            }
            root.addView(snoozeBtn)

            val panicBtn = createTactileButton(
                "Saída de Emergência (FALHOU)",
                "#1E293B",
                "#FCA5A5",
                "#7F1D1D"
            ).apply {
                setOnClickListener {
                    showStructuredPanicDialog(dbPath, alarmId)
                }
            }
            root.addView(panicBtn)
        } else {
            val dismissBtn = createTactileButton(
                "Descartar por Hoje (IGNORADO)",
                "#1E293B",
                "#CBD5E1",
                "#475569"
            ).apply {
                setOnClickListener {
                    recordAndClose(dbPath, alarmId, "IGNORADO")
                }
            }
            root.addView(dismissBtn)
        }
    }

    private fun ensureCameraPermissionAndStart() {
        val granted = ContextCompat.checkSelfPermission(
            this,
            Manifest.permission.CAMERA
        ) == PackageManager.PERMISSION_GRANTED

        if (granted) {
            retryPermissionBtn?.visibility = View.GONE
            startCameraSession(currentDbPath, currentAlarmId, currentTargetLabel)
        } else {
            telemetryView?.text = "Permissão de câmara necessária para validar a Prova de Ação."
            retryPermissionBtn?.visibility = View.VISIBLE
            ActivityCompat.requestPermissions(
                this,
                arrayOf(Manifest.permission.CAMERA),
                REQUEST_CAMERA_PERMISSION
            )
        }
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == REQUEST_CAMERA_PERMISSION) {
            if (grantResults.isNotEmpty() && grantResults[0] == PackageManager.PERMISSION_GRANTED) {
                retryPermissionBtn?.visibility = View.GONE
                startCameraSession(currentDbPath, currentAlarmId, currentTargetLabel)
            } else {
                telemetryView?.text = "Permissão de câmara negada. Conceda permissão ou use Saída de Pânico."
                retryPermissionBtn?.visibility = View.VISIBLE
            }
        }
    }

    @SuppressLint("UnsafeOptInUsageError")
    private fun startCameraSession(dbPath: String, alarmId: String, targetLabel: String) {
        val currentPreview = previewView ?: return
        val executor = cameraExecutor ?: return

        hasCompletedProof.set(false)

        val tracker = ProofFrameTracker(
            targetLabel = targetLabel,
            confidenceThreshold = CONFIDENCE_THRESHOLD,
            requiredFrames = REQUIRED_FRAMES
        )
        val labeler = ImageLabeling.getClient(ImageLabelerOptions.DEFAULT_OPTIONS)
        imageLabeler = labeler

        val cameraProviderFuture = ProcessCameraProvider.getInstance(this)
        cameraProviderFuture.addListener({
            if (isFinishing || isDestroyed || imageLabeler !== labeler) {
                return@addListener
            }
            val provider = try {
                cameraProviderFuture.get()
            } catch (t: Throwable) {
                runOnUiThread {
                    telemetryView?.text = "Erro ao iniciar CameraX: ${t.message}"
                }
                return@addListener
            }
            cameraProvider = provider

            val preview = Preview.Builder().build().also {
                it.setSurfaceProvider(currentPreview.surfaceProvider)
            }

            val resolutionSelector = ResolutionSelector.Builder()
                .setResolutionStrategy(
                    ResolutionStrategy(
                        Size(1280, 720),
                        ResolutionStrategy.FALLBACK_RULE_CLOSEST_HIGHER_THEN_LOWER
                    )
                )
                .build()

            val imageAnalysis = ImageAnalysis.Builder()
                .setResolutionSelector(resolutionSelector)
                .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                .build()

            imageAnalysis.setAnalyzer(executor) { imageProxy ->
                processFrame(
                    imageProxy = imageProxy,
                    executor = executor,
                    labeler = labeler,
                    tracker = tracker,
                    dbPath = dbPath,
                    alarmId = alarmId
                )
            }

            try {
                provider.unbindAll()
                provider.bindToLifecycle(
                    this,
                    CameraSelector.DEFAULT_BACK_CAMERA,
                    preview,
                    imageAnalysis
                )
            } catch (t: Throwable) {
                runOnUiThread {
                    telemetryView?.text = "Falha ao abrir câmara traseira: ${t.message}"
                }
            }
        }, ContextCompat.getMainExecutor(this))
    }

    @SuppressLint("UnsafeOptInUsageError")
    private fun processFrame(
        imageProxy: ImageProxy,
        executor: ExecutorService,
        labeler: ImageLabeler,
        tracker: ProofFrameTracker,
        dbPath: String,
        alarmId: String
    ) {
        if (hasCompletedProof.get()) {
            imageProxy.close()
            return
        }

        val mediaImage = imageProxy.image
        if (mediaImage == null) {
            imageProxy.close()
            return
        }

        val rotationDegrees = imageProxy.imageInfo.rotationDegrees
        val inputImage = InputImage.fromMediaImage(mediaImage, rotationDegrees)

        labeler.process(inputImage)
            .addOnSuccessListener(executor) { labels ->
                if (hasCompletedProof.get()) {
                    return@addOnSuccessListener
                }
                val candidates = labels.map { it.text to it.confidence }
                when (val step = tracker.onFrameLabels(candidates)) {
                    is ProofStepResult.Searching -> {
                        val candidateSummary = if (step.bestCandidateLabel != null && step.bestCandidateConfidence != null) {
                            val pct = String.format(Locale.US, "%.0f%%", step.bestCandidateConfidence * 100f)
                            "${step.bestCandidateLabel} ($pct)"
                        } else {
                            "—"
                        }
                        runOnUiThread {
                            telemetryView?.text =
                                "Progresso: ${step.consecutiveHits}/$REQUIRED_FRAMES frames | Detetado: $candidateSummary"
                        }
                    }

                    is ProofStepResult.Validated -> {
                        if (hasCompletedProof.compareAndSet(false, true)) {
                            val pct = String.format(Locale.US, "%.0f%%", step.confidence * 100f)
                            runOnUiThread {
                                telemetryView?.text =
                                    "Validado! ${step.matchedLabel} ($pct) — Gravando prova..."
                            }
                            val now = System.currentTimeMillis()
                            val savedPath = saveValidatedFrameJpeg(imageProxy, rotationDegrees, alarmId, now)
                            runOnUiThread {
                                recordProofAndClose(
                                    dbPath = dbPath,
                                    alarmId = alarmId,
                                    now = now,
                                    proofPath = savedPath,
                                    matchedLabel = step.matchedLabel,
                                    confidence = step.confidence
                                )
                            }
                        }
                    }
                }
            }
            .addOnCompleteListener(executor) {
                imageProxy.close()
            }
    }

    private fun saveValidatedFrameJpeg(
        imageProxy: ImageProxy,
        rotationDegrees: Int,
        alarmId: String,
        nowEpochMs: Long
    ): String? {
        return try {
            val rawBitmap = imageProxy.toBitmap()
            val finalBitmap = if (rotationDegrees != 0) {
                val matrix = Matrix().apply { postRotate(rotationDegrees.toFloat()) }
                val rotated = Bitmap.createBitmap(
                    rawBitmap,
                    0,
                    0,
                    rawBitmap.width,
                    rawBitmap.height,
                    matrix,
                    true
                )
                if (rotated !== rawBitmap) {
                    rawBitmap.recycle()
                }
                rotated
            } else {
                rawBitmap
            }

            val proofsDir = File(filesDir, "proofs")
            if (!proofsDir.exists()) {
                proofsDir.mkdirs()
            }
            val safeAlarmId = alarmId.replace(Regex("[^a-zA-Z0-9_-]"), "_")
            val outFile = File(proofsDir, "proof_${safeAlarmId}_${nowEpochMs}.jpg")
            FileOutputStream(outFile).use { fos ->
                finalBitmap.compress(Bitmap.CompressFormat.JPEG, 85, fos)
                fos.flush()
            }
            finalBitmap.recycle()
            outFile.absolutePath
        } catch (_: Throwable) {
            null
        }
    }

    private fun showStructuredPanicDialog(dbPath: String, alarmId: String) {
        dismissPanicDialog()
        AlarmStateManager.setPanicActive(true)

        val dialogLayout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(48, 32, 48, 32)
        }

        val timerView = TextView(this).apply {
            textSize = 18f
            gravity = Gravity.CENTER
            text = "Aguarde: 03:00"
            setPadding(0, 0, 0, 16)
        }
        dialogLayout.addView(timerView)

        val instructionView = TextView(this).apply {
            textSize = 14f
            text = "Para confirmar a saída de pânico, digite exatamente a frase abaixo após o término do cronômetro:\n\n\"$COMMITMENT_PHRASE\""
            setPadding(0, 0, 0, 16)
        }
        dialogLayout.addView(instructionView)

        val inputField = EditText(this).apply {
            hint = "Digite a frase aqui..."
            isEnabled = false
            textSize = 14f
        }
        dialogLayout.addView(inputField)

        val confirmBtn = Button(this).apply {
            text = "Confirmar Saída de Pânico (FALHOU)"
            isEnabled = false
        }
        dialogLayout.addView(confirmBtn)

        val cancelBtn = Button(this).apply {
            text = "Voltar e Cumprir Alarme"
            setOnClickListener {
                dismissPanicDialog()
            }
        }
        dialogLayout.addView(cancelBtn)

        inputField.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {
                val matches = s?.toString()?.trim() == COMMITMENT_PHRASE
                confirmBtn.isEnabled = matches && (inputField.isEnabled)
            }
            override fun afterTextChanged(s: Editable?) {}
        })

        confirmBtn.setOnClickListener {
            dismissPanicDialog()
            recordAndClose(dbPath, alarmId, "FALHOU")
        }

        val dialog = AlertDialog.Builder(this)
            .setTitle("Mecânica de Pânico (Penalidade)")
            .setView(dialogLayout)
            .setCancelable(false)
            .create()

        panicDialog = dialog
        dialog.show()

        panicTimer = object : CountDownTimer(PANIC_COUNTDOWN_MS, 1000L) {
            override fun onTick(millisUntilFinished: Long) {
                val totalSeconds = millisUntilFinished / 1000L
                val mins = totalSeconds / 60
                val secs = totalSeconds % 60
                timerView.text = String.format(Locale.US, "Aguarde: %02d:%02d", mins, secs)
            }

            override fun onFinish() {
                timerView.text = "Tempo concluído! Digite a frase para liberar a confirmação."
                inputField.isEnabled = true
                if (inputField.text.toString().trim() == COMMITMENT_PHRASE) {
                    confirmBtn.isEnabled = true
                }
            }
        }.start()
    }

    private fun recordProofAndClose(
        dbPath: String,
        alarmId: String,
        now: Long,
        proofPath: String?,
        matchedLabel: String,
        confidence: Float
    ) {
        dismissPanicDialog()
        stopCameraSession()
        stopAlarmAudioService()
        AlarmStateManager.onAlarmDismissed(alarmId)
        try {
            ffiRecordOutcomeWithProof(
                dbPath,
                alarmId,
                "CUMPRIDO",
                now,
                proofPath,
                matchedLabel,
                confidence
            )
            val nextMs = ffiRescheduleAfterTrigger(dbPath, alarmId, now)
            if (nextMs != null && nextMs > 0L) {
                AlarmScheduler.scheduleExactAlarm(this, alarmId, nextMs)
            }
        } catch (_: Throwable) {
        }
        finish()
    }

    private fun snoozeAndClose(dbPath: String, alarmId: String) {
        dismissPanicDialog()
        stopCameraSession()
        stopAlarmAudioService()
        AlarmStateManager.onAlarmDismissed(alarmId)
        val now = System.currentTimeMillis()
        try {
            ffiRecordOutcome(dbPath, alarmId, "ADIADO", now)
            AlarmScheduler.scheduleExactAlarm(this, alarmId, now + SNOOZE_DURATION_MS)
        } catch (_: Throwable) {
        }
        finish()
    }

    private fun recordAndClose(dbPath: String, alarmId: String, outcome: String) {
        dismissPanicDialog()
        stopCameraSession()
        stopAlarmAudioService()
        AlarmStateManager.onAlarmDismissed(alarmId)
        val now = System.currentTimeMillis()
        try {
            ffiRecordOutcome(dbPath, alarmId, outcome, now)
            val nextMs = ffiRescheduleAfterTrigger(dbPath, alarmId, now)
            if (nextMs != null && nextMs > 0L) {
                AlarmScheduler.scheduleExactAlarm(this, alarmId, nextMs)
            }
        } catch (_: Throwable) {
        }
        finish()
    }

    private fun enableShowWhenLocked() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O_MR1) {
            setShowWhenLocked(true)
            setTurnScreenOn(true)
            val kgm = getSystemService(Context.KEYGUARD_SERVICE) as? KeyguardManager
            kgm?.requestDismissKeyguard(this, null)
        } else {
            @Suppress("DEPRECATION")
            window.addFlags(
                WindowManager.LayoutParams.FLAG_SHOW_WHEN_LOCKED or
                    WindowManager.LayoutParams.FLAG_TURN_SCREEN_ON or
                    WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON
            )
        }
    }
}
