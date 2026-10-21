package com.crabcheck.alarmlib

import uniffi.crabcheck_core_ffi.ffiCuratedSynonymsFor

sealed class ProofStepResult {
    data class Searching(
        val consecutiveHits: Int,
        val bestCandidateLabel: String?,
        val bestCandidateConfidence: Float?
    ) : ProofStepResult()

    data class Validated(
        val matchedLabel: String,
        val confidence: Float,
        val consecutiveHits: Int
    ) : ProofStepResult()
}

class ProofFrameTracker(
    val targetLabel: String,
    val confidenceThreshold: Float = 0.70f,
    val requiredFrames: Int = 5
) {
    private val acceptedSynonyms: Set<String> =
        ffiCuratedSynonymsFor(targetLabel)
            .map { it.trim().lowercase() }
            .filter { it.isNotEmpty() }
            .toSet()

    private var consecutiveHits: Int = 0

    fun onFrameLabels(candidates: List<Pair<String, Float>>): ProofStepResult {
        val bestOverall = candidates.maxByOrNull { it.second }
        val matchingCandidate = candidates
            .filter { (label, conf) ->
                conf >= confidenceThreshold && acceptedSynonyms.contains(label.trim().lowercase())
            }
            .maxByOrNull { it.second }

        return if (matchingCandidate != null) {
            consecutiveHits += 1
            if (consecutiveHits >= requiredFrames) {
                ProofStepResult.Validated(
                    matchedLabel = matchingCandidate.first,
                    confidence = matchingCandidate.second,
                    consecutiveHits = consecutiveHits
                )
            } else {
                ProofStepResult.Searching(
                    consecutiveHits = consecutiveHits,
                    bestCandidateLabel = matchingCandidate.first,
                    bestCandidateConfidence = matchingCandidate.second
                )
            }
        } else {
            consecutiveHits = 0
            ProofStepResult.Searching(
                consecutiveHits = 0,
                bestCandidateLabel = bestOverall?.first,
                bestCandidateConfidence = bestOverall?.second
            )
        }
    }

    fun reset() {
        consecutiveHits = 0
    }
}
