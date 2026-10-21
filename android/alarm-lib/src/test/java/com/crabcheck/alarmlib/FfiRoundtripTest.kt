package com.crabcheck.alarmlib

import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import uniffi.crabcheck_core_ffi.FfiAlarmDto
import uniffi.crabcheck_core_ffi.ffiGetAlarm
import uniffi.crabcheck_core_ffi.ffiListOutcomes
import uniffi.crabcheck_core_ffi.ffiRecordOutcome
import uniffi.crabcheck_core_ffi.ffiUpsertAlarm

class FfiRoundtripTest {
    @get:Rule
    val tempFolder = TemporaryFolder()

    @Test
    fun uniffiRoundtripFromKotlin() {
        val dbFile = tempFolder.newFile("spike_kotlin_test.db")
        val dbPath = dbFile.absolutePath

        val dto = FfiAlarmDto(
            id = "spike-kt-01",
            title = "Leitura às 22:00",
            level = 3,
            targetLabel = "Book",
            triggerAtEpochMs = 1_700_000_100_000L
        )

        ffiUpsertAlarm(dbPath, dto)
        val loaded = ffiGetAlarm(dbPath, "spike-kt-01")
        assertEquals(dto, loaded)

        ffiRecordOutcome(dbPath, "spike-kt-01", "CUMPRIDO", 1_700_000_200_000L)
        val outcomes = ffiListOutcomes(dbPath, "spike-kt-01")
        assertEquals(1, outcomes.size)
        assertEquals("CUMPRIDO", outcomes[0].outcome)
    }
}
