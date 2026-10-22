package com.crabcheck.alarmlib

import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference

object AlarmStateManager {
    private val currentAlarmId = AtomicReference<String?>(null)
    private val currentAlarmLevel = AtomicInteger(0)
    private val panicActive = AtomicBoolean(false)

    fun onAlarmTriggered(alarmId: String, level: Int) {
        currentAlarmId.set(alarmId)
        currentAlarmLevel.set(level)
        panicActive.set(false)
    }

    fun onAlarmDismissed(alarmId: String) {
        if (currentAlarmId.get() == alarmId) {
            currentAlarmId.set(null)
            currentAlarmLevel.set(0)
            panicActive.set(false)
        }
    }

    fun setPanicActive(active: Boolean) {
        panicActive.set(active)
    }

    fun isPanicActive(): Boolean = panicActive.get()

    fun getActiveAlarmId(): String? = currentAlarmId.get()

    fun getActiveAlarmLevel(): Int = currentAlarmLevel.get()

    fun isN3AlarmActive(): Boolean {
        return currentAlarmId.get() != null && currentAlarmLevel.get() == 3
    }
}
