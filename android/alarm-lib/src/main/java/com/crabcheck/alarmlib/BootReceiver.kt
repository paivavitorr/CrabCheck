package com.crabcheck.alarmlib

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import uniffi.crabcheck_core_ffi.ffiListAlarms
import uniffi.crabcheck_core_ffi.ffiRescheduleAfterTrigger

class BootReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent?) {
        val action = intent?.action ?: return
        when (action) {
            Intent.ACTION_BOOT_COMPLETED,
            Intent.ACTION_MY_PACKAGE_REPLACED,
            Intent.ACTION_TIME_CHANGED,
            Intent.ACTION_TIMEZONE_CHANGED -> {
                rescheduleAllActiveAlarms(context)
            }
        }
    }

    private fun rescheduleAllActiveAlarms(context: Context) {
        val dbPath = AlarmScheduler.dbPath(context)
        val nowEpochMs = System.currentTimeMillis()

        try {
            val alarms = ffiListAlarms(dbPath)
            for (alarm in alarms) {
                if (!alarm.enabled) continue

                val targetTriggerMs: Long? = if (alarm.triggerAtEpochMs > nowEpochMs) {
                    alarm.triggerAtEpochMs
                } else {
                    ffiRescheduleAfterTrigger(dbPath, alarm.id, nowEpochMs)
                }

                if (targetTriggerMs != null && targetTriggerMs > nowEpochMs) {
                    AlarmScheduler.scheduleExactAlarm(context, alarm.id, targetTriggerMs)
                }
            }
        } catch (_: Throwable) {
            // Em caso de banco não inicializado ainda, silenciosamente ignorado
        }
    }
}
