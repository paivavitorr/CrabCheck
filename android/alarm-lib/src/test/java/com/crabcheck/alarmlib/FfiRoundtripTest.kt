package com.crabcheck.alarmlib

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import uniffi.crabcheck_core_ffi.FfiAlarmDto
import uniffi.crabcheck_core_ffi.ffiCuratedSynonymsFor
import uniffi.crabcheck_core_ffi.ffiDeleteAlarm
import uniffi.crabcheck_core_ffi.ffiGetAlarm
import uniffi.crabcheck_core_ffi.ffiListAlarms
import uniffi.crabcheck_core_ffi.ffiListOutcomes
import uniffi.crabcheck_core_ffi.ffiMatchesCuratedLabel
import uniffi.crabcheck_core_ffi.ffiRecordOutcome
import uniffi.crabcheck_core_ffi.ffiRecordOutcomeWithProof
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
        assertNull(outcomes[0].proofImagePath)
        assertNull(outcomes[0].matchedLabel)
        assertNull(outcomes[0].confidence)

        ffiRecordOutcomeWithProof(
            dbPath,
            "spike-kt-01",
            "CUMPRIDO",
            now + 250_000L,
            "/data/user/0/com.crabcheck.app/files/proofs/proof_spike-kt-01.jpg",
            "Publication",
            0.84f
        )
        val outcomesAfterProof = ffiListOutcomes(dbPath, "spike-kt-01")
        assertEquals(2, outcomesAfterProof.size)
        val latest = outcomesAfterProof[1]
        assertEquals(
            "/data/user/0/com.crabcheck.app/files/proofs/proof_spike-kt-01.jpg",
            latest.proofImagePath
        )
        assertEquals("Publication", latest.matchedLabel)
        assertNotNull(latest.confidence)
        assertEquals(0.84f, latest.confidence!!, 1e-4f)

        val nextTrigger = ffiRescheduleAfterTrigger(dbPath, "spike-kt-01", now + 250_000L)
        assertNotNull(nextTrigger)
        assertTrue(nextTrigger!! > now + 250_000L)

        // Cooldown unlock and delete after 24h
        val unlockAt = ffiRequestCooldownUnlock(dbPath, "spike-kt-01", now + 300_000L)
        ffiDeleteAlarm(dbPath, "spike-kt-01", unlockAt)
        assertTrue(ffiListAlarms(dbPath).isEmpty())
    }

    @Test
    fun curatedSynonymsAndProofFrameTrackerValidation() {
        assertTrue(ffiMatchesCuratedLabel("Shoe", "Sneakers"))
        assertTrue(ffiMatchesCuratedLabel("Cup", "Mug"))
        assertTrue(ffiMatchesCuratedLabel("Bottle", "Water bottle"))
        assertFalse(ffiMatchesCuratedLabel("Shoe", "Laptop"))
        assertTrue(ffiCuratedSynonymsFor("Book").contains("publication"))

        val tracker = ProofFrameTracker(
            targetLabel = "Shoe",
            confidenceThreshold = 0.70f,
            requiredFrames = 5
        )

        // 4 consecutive matching frames
        for (expectedHits in 1..4) {
            val step = tracker.onFrameLabels(
                listOf("Room" to 0.90f, "Sneakers" to 0.78f)
            )
            assertTrue(step is ProofStepResult.Searching)
            val searching = step as ProofStepResult.Searching
            assertEquals(expectedHits, searching.consecutiveHits)
            assertEquals("Sneakers", searching.bestCandidateLabel)
        }

        // 1 frame below threshold or unrelated resets counter to 0
        val missStep = tracker.onFrameLabels(
            listOf("Sneakers" to 0.65f, "Chair" to 0.88f)
        )
        assertTrue(missStep is ProofStepResult.Searching)
        assertEquals(0, (missStep as ProofStepResult.Searching).consecutiveHits)

        // 5 consecutive matching frames trigger Validated
        for (i in 1..4) {
            val s = tracker.onFrameLabels(listOf("Footwear" to 0.82f))
            assertTrue(s is ProofStepResult.Searching)
            assertEquals(i, (s as ProofStepResult.Searching).consecutiveHits)
        }
        val finalStep = tracker.onFrameLabels(listOf("Boot" to 0.91f))
        assertTrue(finalStep is ProofStepResult.Validated)
        val validated = finalStep as ProofStepResult.Validated
        assertEquals("Boot", validated.matchedLabel)
        assertEquals(0.91f, validated.confidence, 1e-4f)
        assertEquals(5, validated.consecutiveHits)
    }

    @Test
    fun alarmStateManagerTracking() {
        assertFalse(AlarmStateManager.isN3AlarmActive())
        assertNull(AlarmStateManager.getActiveAlarmId())
        assertEquals(0, AlarmStateManager.getActiveAlarmLevel())

        AlarmStateManager.onAlarmTriggered("alarm-n2", 2)
        assertEquals("alarm-n2", AlarmStateManager.getActiveAlarmId())
        assertEquals(2, AlarmStateManager.getActiveAlarmLevel())
        assertFalse(AlarmStateManager.isN3AlarmActive())

        AlarmStateManager.onAlarmTriggered("alarm-n3", 3)
        assertEquals("alarm-n3", AlarmStateManager.getActiveAlarmId())
        assertEquals(3, AlarmStateManager.getActiveAlarmLevel())
        assertTrue(AlarmStateManager.isN3AlarmActive())

        AlarmStateManager.setPanicActive(true)
        assertTrue(AlarmStateManager.isPanicActive())

        AlarmStateManager.onAlarmDismissed("alarm-n3")
        assertFalse(AlarmStateManager.isN3AlarmActive())
        assertNull(AlarmStateManager.getActiveAlarmId())
        assertFalse(AlarmStateManager.isPanicActive())
    }
}
