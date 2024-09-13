package com.ammaraskar.intent.fuzz

import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.File

@Serializable
class IntentTemplate(
    val receiver_type: String,
    val component: String,
    val actions: Collection<String>,
    val categories: Collection<String>,
    val known_extras_keys: Map<String, String>,
) {
    // Save this template to a json file in a given directory
    fun saveToFile(outputDir: File) {
        val jsonFormat = Json { prettyPrint = true }
        outputDir.mkdirs()
        File(outputDir, "${getActivityName()}.json").writeText(jsonFormat.encodeToString(this))
    }

    fun getActivityName(): String {
        return component.substringAfterLast("/")
    }
}
