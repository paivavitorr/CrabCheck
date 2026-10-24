package com.crabcheck.alarmlib

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import uniffi.crabcheck_core_ffi.ffiGetAlarm
import java.util.Locale

class AlarmReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val alarmId = intent.getStringExtra(AlarmScheduler.EXTRA_ALARM_ID) ?: "spike-001"
        val isPreAlarm = intent.getBooleanExtra(AlarmScheduler.EXTRA_IS_PRE_ALARM, false)
        val dbPath = AlarmScheduler.dbPath(context)

        if (isPreAlarm) {
            handlePreAlarm(context, dbPath, alarmId)
            return
        }

        // Se o alarme principal disparou, remove qualquer notificação prévia de 15 minutos pendente
        try {
            val nm = ContextCompat.getSystemService(context, NotificationManager::class.java)
            nm?.cancel(AlarmScheduler.preAlarmNotificationId(alarmId))
        } catch (_: Throwable) {
        }

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

    private fun handlePreAlarm(context: Context, dbPath: String, alarmId: String) {
        try {
            val dto = ffiGetAlarm(dbPath, alarmId)
            if (!dto.enabled) {
                return
            }

            // Garante o canal de notificação com alta prioridade para o lembrete prévio
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                val channel = NotificationChannel(
                    AlarmScheduler.PRE_ALARM_CHANNEL_ID,
                    "Lembretes Prévios (Leitura e Estudo)",
                    NotificationManager.IMPORTANCE_HIGH
                ).apply {
                    description = "Avisos 15 minutos antes do horário de estudo e leitura"
                    enableVibration(true)
                    lockscreenVisibility = Notification.VISIBILITY_PUBLIC
                }
                val nm = context.getSystemService(NotificationManager::class.java)
                nm?.createNotificationChannel(channel)
            }

            val timeStr = String.format(Locale.getDefault(), "%02d:%02d", dto.hour.toInt(), dto.minute.toInt())
            val targetLabel = dto.targetLabel.ifBlank { "livro ou material" }
            val titleText = "📖 ${dto.title} em 15 min ($timeStr)"
            val shortText = "Hora de se preparar! Tenha seu $targetLabel por perto."
            val longText = "Faltam 15 minutos para iniciar seu compromisso de ${dto.title} ($timeStr). Separe seu $targetLabel para a validação fotográfica."

            val launchIntent = context.packageManager.getLaunchIntentForPackage(context.packageName)
                ?: Intent(context, LockActivity::class.java).apply {
                    putExtra(AlarmScheduler.EXTRA_ALARM_ID, alarmId)
                }
            launchIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)

            val openPi = PendingIntent.getActivity(
                context,
                ("open_pre_$alarmId").hashCode(),
                launchIntent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )

            val notification = NotificationCompat.Builder(context, AlarmScheduler.PRE_ALARM_CHANNEL_ID)
                .setSmallIcon(android.R.drawable.ic_popup_reminder)
                .setContentTitle(titleText)
                .setContentText(shortText)
                .setStyle(NotificationCompat.BigTextStyle().bigText(longText))
                .setPriority(NotificationCompat.PRIORITY_HIGH)
                .setCategory(NotificationCompat.CATEGORY_REMINDER)
                .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
                .setAutoCancel(true)
                .setContentIntent(openPi)
                .build()

            val nm = ContextCompat.getSystemService(context, NotificationManager::class.java)
            nm?.notify(AlarmScheduler.preAlarmNotificationId(alarmId), notification)
        } catch (_: Throwable) {
        }
    }
}

