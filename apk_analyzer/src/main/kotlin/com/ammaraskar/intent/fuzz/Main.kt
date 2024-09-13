package com.ammaraskar.intent.fuzz

import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.File

fun main(args: Array<String>) {
    val analyzer = ApkAnalyzer(File(args[0]))

    val jsonFormat = Json { prettyPrint = true }
    println(jsonFormat.encodeToString(analyzer.intentTemplates))

    // Save all intent templates to a given output directory
    analyzer.intentTemplates.forEach { it.saveToFile(File(args[1])) }
}