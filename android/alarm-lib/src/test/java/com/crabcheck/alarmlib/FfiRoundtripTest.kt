package com.crabcheck.alarmlib

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import uniffi.crabcheck_core_ffi.FfiAlarmDto
import uniffi.crabcheck_core_ffi.ffiDeleteAlarm
import uniffi.crabcheck_core_ffi.ffiGetAlarm
import uniffi.crabcheck_core_ffi.ffiListAlarms
import uniffi.crabcheck_core_ffi.ffiListOutcomes
import uniffi.crabcheck_core_ffi.ffiRecordOutcome
import uniffi.crabcheck_core_ffi.ffiRequestCooldownUnlock
import uniffi.crabcheck_core_ffi.ffiRescheduleAfterTrigger
import uniffi.crabcheck_core_ffi.ffiUpsertAlarm

class FfiRoundtripTest {
    @get:Rule
    val tempFolder = TemporaryFolder()

    @Test
    fun uniffiRoundtripFromKotlin() {
        val dbFile = tempFolder.newFile("spike_kotlin_test.db")
        val dbPath = dbFile.absolutePath
        val now = 1_700_000_000_000L

        val dto = FfiAlarmDto(
            id = "spike-kt-01",
            title = "Leitura às 22:00",
            level = 3,
            targetLabel = "Book",
            hour = 22u,
            minute = 0u,
            weekdaysMask = 0b0111_1111u,
            graceWindowMinutes = 30u,
            enabled = true,
            timezone = "Local",
            triggerAtEpochMs = now + 100_000L,
            cooldownUnlockAtEpochMs = null,
            createdAtEpochMs = now,
            updatedAtEpochMs = now
        )

        ffiUpsertAlarm(dbPath, dto)
        val loaded = ffiGetAlarm(dbPath, "spike-kt-01")
        assertEquals(dto, loaded)

        val listed = ffiListAlarms(dbPath)
        assertEquals(1, listed.size)

        ffiRecordOutcome(dbPath, "spike-kt-01", "CUMPRIDO", now + 200_000L)
        val outcomes = ffiListOutcomes(dbPath, "spike-kt-01")
        assertEquals(1, outcomes.size)
        assertEquals("CUMPRIDO", outcomes[0].outcome)

        val nextTrigger = ffiRescheduleAfterTrigger(dbPath, "spike-kt-01", now + 200_000L)
        assertNotNull(nextTrigger)
        assertTrue(nextTrigger!! > now + 200_000L)

        // Cooldown unlock and delete after 24h
        val unlockAt = ffiRequestCooldownUnlock(dbPath, "spike-kt-01", now + 300_000L)
        ffiDeleteAlarm(dbPath, "spike-kt-01", unlockAt)
        assertTrue(ffiListAlarms(dbPath).isEmpty())
    }
}
