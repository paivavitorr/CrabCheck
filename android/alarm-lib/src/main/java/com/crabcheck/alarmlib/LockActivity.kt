package com.crabcheck.alarmlib

import android.Manifest
import android.annotation.SuppressLint
import android.app.KeyguardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Matrix
import android.os.Build
import android.os.Bundle
import android.util.Size
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView
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
    }

    private var cameraExecutor: ExecutorService? = null
    private var cameraProvider: ProcessCameraProvider? = null
    private var imageLabeler: ImageLabeler? = null
    private val hasCompletedProof = AtomicBoolean(false)

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
            setIntent(intent)
            renderAlarmFromIntent(intent)
        }
    }

    override fun onDestroy() {
        stopCameraSession()
        cameraExecutor?.shutdown()
        cameraExecutor = null
        super.onDestroy()
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
            gravity = Gravity.CENTER
            setPadding(40, 40, 40, 40)
        }

        val titleView = TextView(this).apply {
            text = "CRABCHECK — ALARME NÍVEL $level"
            textSize = 20f
            gravity = Gravity.CENTER
        }
        root.addView(titleView)

        val detailText = if (dto != null) {
            "Alarme #${dto.id}: ${dto.title}\nNível ${dto.level}" +
                if (level == 3) " | Objeto exigido: $targetLabel" else ""
        } else {
            "Alarme #$alarmId\nNível $level | Objeto exigido: $targetLabel"
        }

        val detailView = TextView(this).apply {
            text = detailText
            textSize = 15f
            gravity = Gravity.CENTER
            setPadding(0, 16, 0, 24)
        }
        root.addView(detailView)

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
            text = "Aponte a câmara para: $targetLabel (≥70% por $REQUIRED_FRAMES frames)"
            textSize = 16f
            gravity = Gravity.CENTER
            setPadding(0, 0, 0, 16)
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
            textSize = 14f
            gravity = Gravity.CENTER
            setPadding(0, 16, 0, 16)
        }
        telemetryView = statusView
        root.addView(statusView)

        val permBtn = Button(this).apply {
            text = "Conceder Permissão de Câmara"
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

        val panicBtn = Button(this).apply {
            text = "Saída de Pânico (FALHOU)"
            setOnClickListener {
                recordAndClose(dbPath, alarmId, "FALHOU")
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
        val completeBtn = Button(this).apply {
            text = "Concluir Alarme (CUMPRIDO)"
            setOnClickListener {
                recordAndClose(dbPath, alarmId, "CUMPRIDO")
            }
        }
        root.addView(completeBtn)

        if (level == 2) {
            val snoozeBtn = Button(this).apply {
                text = "Adiar 5 min (ADIADO)"
                setOnClickListener {
                    snoozeAndClose(dbPath, alarmId)
                }
            }
            root.addView(snoozeBtn)

            val panicBtn = Button(this).apply {
                text = "Saída de Pânico (FALHOU)"
                setOnClickListener {
                    recordAndClose(dbPath, alarmId, "FALHOU")
                }
            }
            root.addView(panicBtn)
        } else {
            val dismissBtn = Button(this).apply {
                text = "Descartar (IGNORADO)"
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

    private fun recordProofAndClose(
        dbPath: String,
        alarmId: String,
        now: Long,
        proofPath: String?,
        matchedLabel: String,
        confidence: Float
    ) {
        stopCameraSession()
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
        stopCameraSession()
        val now = System.currentTimeMillis()
        try {
            ffiRecordOutcome(dbPath, alarmId, "ADIADO", now)
            AlarmScheduler.scheduleExactAlarm(this, alarmId, now + SNOOZE_DURATION_MS)
        } catch (_: Throwable) {
        }
        finish()
    }

    private fun recordAndClose(dbPath: String, alarmId: String, outcome: String) {
        stopCameraSession()
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
