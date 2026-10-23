package com.crabcheck.alarmlib

import uniffi.crabcheck_core_ffi.ffiCuratedSynonymsFor

sealed class ProofStepResult {
    data class Searching(
        val consecutiveHits: Int,
        val targetLabel: String,
        val matchedTargetLabel: String?,
        val matchedTargetConfidence: Float?,
        val topSceneLabel: String?,
        val topSceneConfidence: Float?
    ) : ProofStepResult() {
        val bestCandidateLabel: String?
            get() = matchedTargetLabel ?: topSceneLabel
        val bestCandidateConfidence: Float?
            get() = matchedTargetConfidence ?: topSceneConfidence
    }

    data class Validated(
        val matchedLabel: String,
        val confidence: Float,
        val consecutiveHits: Int
    ) : ProofStepResult()
}

class ProofFrameTracker(
    val targetLabel: String,
    val confidenceThreshold: Float = 0.58f,
    val requiredFrames: Int = 3
) {
    private val acceptedSynonyms: Set<String> =
        ffiCuratedSynonymsFor(targetLabel)
            .map { it.trim().lowercase() }
            .filter { it.isNotEmpty() }
            .toSet()

    private var consecutiveHits: Int = 0

    private fun isMatch(label: String): Boolean {
        val norm = label.trim().lowercase()
        return acceptedSynonyms.any { syn ->
            norm == syn ||
                (syn.length >= 4 && norm.contains(syn)) ||
                (norm.length >= 4 && syn.contains(norm))
        }
    }

    fun onFrameLabels(candidates: List<Pair<String, Float>>): ProofStepResult {
        val bestOverall = candidates.maxByOrNull { it.second }
        val bestTargetMatch = candidates
            .filter { (label, _) -> isMatch(label) }
            .maxByOrNull { it.second }

        val isTargetQualified = bestTargetMatch != null && bestTargetMatch.second >= confidenceThreshold

        return if (isTargetQualified) {
            consecutiveHits += 1
            if (consecutiveHits >= requiredFrames) {
                ProofStepResult.Validated(
                    matchedLabel = bestTargetMatch!!.first,
                    confidence = bestTargetMatch.second,
                    consecutiveHits = consecutiveHits
                )
            } else {
                ProofStepResult.Searching(
                    consecutiveHits = consecutiveHits,
                    targetLabel = targetLabel,
                    matchedTargetLabel = bestTargetMatch!!.first,
                    matchedTargetConfidence = bestTargetMatch.second,
                    topSceneLabel = bestOverall?.first,
                    topSceneConfidence = bestOverall?.second
                )
            }
        } else {
            consecutiveHits = 0
            ProofStepResult.Searching(
                consecutiveHits = 0,
                targetLabel = targetLabel,
                matchedTargetLabel = bestTargetMatch?.first,
                matchedTargetConfidence = bestTargetMatch?.second,
                topSceneLabel = bestOverall?.first,
                topSceneConfidence = bestOverall?.second
            )
        }
    }

    fun reset() {
        consecutiveHits = 0
    }
}
