package com.crabcheck.alarmlib

import android.accessibilityservice.AccessibilityService
import android.content.Intent
import android.view.accessibility.AccessibilityEvent

class AlarmAccessibilityService : AccessibilityService() {

    companion object {
        private val EXEMPT_PACKAGES = setOf(
            "com.android.server.telecom",
            "com.android.phone",
            "com.google.android.dialer",
            "com.samsung.android.dialer",
            "com.samsung.android.incallui",
            "com.android.incallui",
            "com.android.emergency",
            "android.inputmethodservice",
            "com.google.android.inputmethod.latin",
            "com.samsung.android.honeyboard",
            "com.android.permissioncontroller",
            "com.google.android.permissioncontroller"
        )
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        if (event == null) return

        if (!AlarmStateManager.isN3AlarmActive()) return
        if (AlarmStateManager.isPanicActive()) return

        val packageName = event.packageName?.toString() ?: return
        val className = event.className?.toString() ?: ""

        if (packageName == applicationContext.packageName) {
            return
        }

        if (isExemptPackage(packageName)) {
            return
        }

        if (className == "com.crabcheck.alarmlib.LockActivity") {
            return
        }

        if (event.eventType == AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED) {
            bringLockActivityToFront()
        }
    }

    private fun isExemptPackage(packageName: String): Boolean {
        if (EXEMPT_PACKAGES.contains(packageName)) return true
        val lower = packageName.lowercase()
        return lower.contains("dialer") ||
            lower.contains("telecom") ||
            lower.contains("emergency") ||
            lower.contains("incallui") ||
            lower.contains("inputmethod") ||
            lower.contains("keyboard") ||
            lower.contains("permissioncontroller")
    }

    private fun bringLockActivityToFront() {
        val alarmId = AlarmStateManager.getActiveAlarmId() ?: return
        val lockIntent = Intent(this, LockActivity::class.java).apply {
            putExtra(AlarmScheduler.EXTRA_ALARM_ID, alarmId)
            addFlags(
                Intent.FLAG_ACTIVITY_NEW_TASK or
                    Intent.FLAG_ACTIVITY_REORDER_TO_FRONT or
                    Intent.FLAG_ACTIVITY_SINGLE_TOP
            )
        }
        startActivity(lockIntent)
    }

    override fun onInterrupt() {
    }
}
