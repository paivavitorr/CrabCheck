package com.crabcheck.alarmlib

import android.app.AlarmManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import java.io.File

object AlarmScheduler {
    const val EXTRA_ALARM_ID = "com.crabcheck.alarmlib.EXTRA_ALARM_ID"

    @JvmStatic
    fun dbPath(context: Context): String {
        return File(context.filesDir, "crabcheck.db").absolutePath
    }

    @JvmStatic
    fun scheduleExactAlarm(context: Context, alarmId: String, triggerAtEpochMs: Long) {
        val alarmManager = context.getSystemService(Context.ALARM_SERVICE) as AlarmManager

        val receiverIntent = Intent(context, AlarmReceiver::class.java).apply {
            action = "com.crabcheck.alarmlib.ACTION_TRIGGER_ALARM_$alarmId"
            putExtra(EXTRA_ALARM_ID, alarmId)
        }

        val operationPi = PendingIntent.getBroadcast(
            context,
            alarmId.hashCode(),
            receiverIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val showIntent = Intent(context, LockActivity::class.java).apply {
            putExtra(EXTRA_ALARM_ID, alarmId)
        }
        val showPi = PendingIntent.getActivity(
            context,
            alarmId.hashCode(),
            showIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val clockInfo = AlarmManager.AlarmClockInfo(triggerAtEpochMs, showPi)
        alarmManager.setAlarmClock(clockInfo, operationPi)
    }
}
