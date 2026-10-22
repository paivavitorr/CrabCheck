package com.crabcheck.alarmlib

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import androidx.core.content.ContextCompat
import uniffi.crabcheck_core_ffi.ffiGetAlarm

class AlarmReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val alarmId = intent.getStringExtra(AlarmScheduler.EXTRA_ALARM_ID) ?: "spike-001"
        val dbPath = AlarmScheduler.dbPath(context)

        val level = try {
            val dto = ffiGetAlarm(dbPath, alarmId)
            dto.level
        } catch (_: Throwable) {
            3
        }

        AlarmStateManager.onAlarmTriggered(alarmId, level)

        val serviceIntent = Intent(context, AlarmAudioService::class.java).apply {
            putExtra(AlarmAudioService.EXTRA_ALARM_ID, alarmId)
        }
        ContextCompat.startForegroundService(context, serviceIntent)

        val lockIntent = Intent(context, LockActivity::class.java).apply {
            addFlags(
                Intent.FLAG_ACTIVITY_NEW_TASK or
                    Intent.FLAG_ACTIVITY_CLEAR_TOP or
                    Intent.FLAG_ACTIVITY_SINGLE_TOP
            )
            putExtra(AlarmScheduler.EXTRA_ALARM_ID, alarmId)
        }
        try {
            context.startActivity(lockIntent)
        } catch (_: Throwable) {
            // Em caso de restrição de background, o fullScreenIntent do serviço foreground assume
        }
    }
}

