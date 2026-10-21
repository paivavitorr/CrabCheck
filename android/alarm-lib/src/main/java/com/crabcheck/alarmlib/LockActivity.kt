package com.crabcheck.alarmlib

import android.app.KeyguardManager
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.view.Gravity
import android.view.WindowManager
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import uniffi.crabcheck_core_ffi.ffiGetAlarm
import uniffi.crabcheck_core_ffi.ffiRecordOutcome

class LockActivity : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
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

    private fun renderAlarmFromIntent(currentIntent: Intent) {
        val alarmId = currentIntent.getStringExtra(AlarmScheduler.EXTRA_ALARM_ID) ?: "spike-001"
        val dbPath = AlarmScheduler.dbPath(this)

        val alarmInfo = try {
            val dto = ffiGetAlarm(dbPath, alarmId)
            "Alarme #${dto.id}\n${dto.title}\nNível ${dto.level} | Objeto: ${dto.targetLabel}"
        } catch (t: Throwable) {
            "Alarme #$alarmId\n(Erro ao ler UniFFI: ${t.message})"
        }

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            setPadding(48, 48, 48, 48)
        }

        val titleView = TextView(this).apply {
            text = "CRABCHECK — BLOQUEIO DE ALARME (SPIKE)"
            textSize = 20f
            gravity = Gravity.CENTER
        }

        val detailView = TextView(this).apply {
            text = alarmInfo
            textSize = 16f
            gravity = Gravity.CENTER
            setPadding(0, 32, 0, 48)
        }

        val successBtn = Button(this).apply {
            text = "Validar Prova (CUMPRIDO)"
            setOnClickListener {
                recordAndClose(dbPath, alarmId, "CUMPRIDO")
            }
        }

        val panicBtn = Button(this).apply {
            text = "Saída de Pânico (FALHOU)"
            setOnClickListener {
                recordAndClose(dbPath, alarmId, "FALHOU")
            }
        }

        root.addView(titleView)
        root.addView(detailView)
        root.addView(successBtn)
        root.addView(panicBtn)

        setContentView(root)
    }

    private fun recordAndClose(dbPath: String, alarmId: String, outcome: String) {
        try {
            ffiRecordOutcome(dbPath, alarmId, outcome, System.currentTimeMillis())
        } catch (_: Throwable) {
            // Ignorado no spike se falhar
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
