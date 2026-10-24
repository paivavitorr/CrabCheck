package com.crabcheck.alarmlib

import android.Manifest
import android.app.Activity
import android.app.AlarmManager
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import androidx.core.app.ActivityCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import java.io.File

object AlarmScheduler {
    const val EXTRA_ALARM_ID = "com.crabcheck.alarmlib.EXTRA_ALARM_ID"
    private const val REQ_RUNTIME_PERMISSION = 3001

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

    @JvmStatic
    fun cancelExactAlarm(context: Context, alarmId: String) {
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

        alarmManager.cancel(operationPi)
        operationPi.cancel()
    }

    @JvmStatic
    fun checkPermissionsCsv(context: Context): String {
        val exactAlarmOk = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            val am = context.getSystemService(Context.ALARM_SERVICE) as? AlarmManager
            am?.canScheduleExactAlarms() == true
        } else {
            true
        }

        val overlayOk = Settings.canDrawOverlays(context)

        val accessibilityOk = true

        val cameraOk = ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.CAMERA
        ) == PackageManager.PERMISSION_GRANTED

        val notificationsEnabled =
            NotificationManagerCompat.from(context).areNotificationsEnabled()
        val fullScreenOk = if (Build.VERSION.SDK_INT >= 34) {
            val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as? NotificationManager
            nm?.canUseFullScreenIntent() != false
        } else {
            true
        }
        val notificationsOk = notificationsEnabled && fullScreenOk

        val pm = context.getSystemService(Context.POWER_SERVICE) as? PowerManager
        val batteryOk = pm?.isIgnoringBatteryOptimizations(context.packageName) == true

        fun flag(v: Boolean): String = if (v) "1" else "0"

        return buildString {
            append("exact_alarm=").append(flag(exactAlarmOk)).append(",")
            append("overlay=").append(flag(overlayOk)).append(",")
            append("accessibility=").append(flag(accessibilityOk)).append(",")
            append("camera=").append(flag(cameraOk)).append(",")
            append("notifications=").append(flag(notificationsOk)).append(",")
            append("battery=").append(flag(batteryOk))
        }
    }

    @JvmStatic
    fun openPermissionSetting(context: Context, key: String) {
        val pkgUri = Uri.parse("package:${context.packageName}")
        try {
            when (key) {
                "exact_alarm" -> {
                    val intent = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                        Intent(Settings.ACTION_REQUEST_SCHEDULE_EXACT_ALARM, pkgUri)
                    } else {
                        Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, pkgUri)
                    }
                    intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    context.startActivity(intent)
                }

                "overlay" -> {
                    val intent = Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, pkgUri).apply {
                        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    }
                    context.startActivity(intent)
                }

                "accessibility" -> {
                    val intent = Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS).apply {
                        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    }
                    context.startActivity(intent)
                }

                "camera" -> {
                    if (context is Activity) {
                        ActivityCompat.requestPermissions(
                            context,
                            arrayOf(Manifest.permission.CAMERA),
                            REQ_RUNTIME_PERMISSION
                        )
                    } else {
                        val intent = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, pkgUri).apply {
                            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                        }
                        context.startActivity(intent)
                    }
                }

                "notifications" -> {
                    if (Build.VERSION.SDK_INT >= 33 &&
                        context is Activity &&
                        ContextCompat.checkSelfPermission(
                            context,
                            Manifest.permission.POST_NOTIFICATIONS
                        ) != PackageManager.PERMISSION_GRANTED
                    ) {
                        ActivityCompat.requestPermissions(
                            context,
                            arrayOf(Manifest.permission.POST_NOTIFICATIONS),
                            REQ_RUNTIME_PERMISSION
                        )
                    } else if (Build.VERSION.SDK_INT >= 34 &&
                        NotificationManagerCompat.from(context).areNotificationsEnabled() &&
                        (context.getSystemService(Context.NOTIFICATION_SERVICE) as? NotificationManager)
                            ?.canUseFullScreenIntent() == false
                    ) {
                        val intent = Intent(
                            Settings.ACTION_MANAGE_APP_USE_FULL_SCREEN_INTENT,
                            pkgUri
                        ).apply {
                            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                        }
                        context.startActivity(intent)
                    } else {
                        val intent = Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).apply {
                            putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)
                            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                        }
                        context.startActivity(intent)
                    }
                }

                "battery" -> {
                    try {
                        val intent = Intent(
                            Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS,
                            pkgUri
                        ).apply {
                            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                        }
                        context.startActivity(intent)
                    } catch (_: Throwable) {
                        val fallback = Intent(
                            Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS
                        ).apply {
                            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                        }
                        context.startActivity(fallback)
                    }
                }

                else -> {
                    val intent = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, pkgUri).apply {
                        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    }
                    context.startActivity(intent)
                }
            }
        } catch (_: Throwable) {
            try {
                val fallback = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, pkgUri).apply {
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                }
                context.startActivity(fallback)
            } catch (_: Throwable) {
            }
        }
    }
}
