package com.ammaraskar.intent.fuzz

import jadx.api.JadxDecompiler
import javax.xml.parsers.DocumentBuilderFactory

class IntentReceiver(
    val receiverType: String,
    val componentName: String,
    val actions: Collection<String>,
    val categories: Collection<String>,
    val aliasTarget: String?
) {
    override fun equals(other: Any?): Boolean {
        if (other !is IntentReceiver) {
            return false
        }
        return receiverType == other.receiverType &&
                componentName == other.componentName &&
                actions == other.actions &&
                categories == other.categories &&
                aliasTarget == other.aliasTarget
    }
}

fun parseIntentReceiversFromManifest(manifestXML: String, decompiler: JadxDecompiler): List<IntentReceiver> {
    val targets = mutableListOf<IntentReceiver>()

    val documentBuilder = DocumentBuilderFactory.newInstance().newDocumentBuilder()
    val document = documentBuilder.parse(manifestXML.byteInputStream())

    val manifestElement = document.getElementsByTagName("manifest")
    if (manifestElement.length != 1) {
        throw IllegalArgumentException("AndroidManifest contained ${manifestElement.length} manifest elements")
    }
    val packageName = manifestElement.item(0).attributes.getNamedItem("package")?.nodeValue
        ?: throw IllegalArgumentException("<manifest> element did not contain package")

    // Find all <intent-filter> tags and iterate over them.
    val intentFilters = document.getElementsByTagName("intent-filter")
    for (i in 0 until intentFilters.length) {
        val node = intentFilters.item(i)

        val containingComponent = node.parentNode

        // Get the class name of the component containing this intent-filter.
        val intentClass = containingComponent.attributes.getNamedItem("android:name")?.nodeValue
        if (intentClass == null) {
            println("Skipping intent receiver because it doesn't have an android:name attribute")
            continue
        }

        // Check to see if the component is exported. That is, either the "android:exported" attribute is marked as
        // "true" or if it isn't present, it takes a default value of true when there is an <intent-filter>
        val exportedAttribute =
            containingComponent.attributes.getNamedItem("android:exported")?.nodeValue
                ?: "true"
        val isExported = exportedAttribute == "true"

        // Only add to list of targets if it is exported.
        if (!isExported) {
            continue
        }

        // Check if this is an alias.
        val aliasTargetActivity = if (containingComponent.nodeName == "activity-alias") {
            containingComponent.attributes.getNamedItem("android:targetActivity")?.nodeValue
                ?: throw IllegalArgumentException("Manifest has an activity-alias without an android:targetActivity")
        } else {
            null
        }

        val actionNames = mutableListOf<String>()
        val categoryNames = mutableListOf<String>()
        // Gather all the action tags.
        for (j in 0 until node.childNodes.length) {
            val intentFilterChild = node.childNodes.item(j)

            // Check if this is a <category> or an <action>
            if (intentFilterChild.nodeName == "action") {
                actionNames.add(intentFilterChild.attributes.getNamedItem("android:name").nodeValue)
            } else if (intentFilterChild.nodeName == "category") {
                categoryNames.add(intentFilterChild.attributes.getNamedItem("android:name").nodeValue)
            }
        }

        val receiverType = when (containingComponent.nodeName) {
            "activity" -> "Activity"
            "activity-alias" -> "Activity"
            "service" -> continue  // We do not support fuzzing services for now
            "receiver" -> "BroadcastReceiver"
            "provider" -> continue  // We do not support content providers for now
            else -> throw IllegalArgumentException("Unknown component type: ${containingComponent.nodeName}")
        }

        // Create the intent receiver object.
        val intentReceiver = IntentReceiver(
            receiverType = receiverType,
            componentName = "$packageName/$intentClass",
            actions = actionNames,
            categories = categoryNames,
            aliasTarget = aliasTargetActivity
        )

        // Only add alias if it has new actions or categories.
        if (aliasTargetActivity != null) {
            val existingTargets = targets.filter {
                it.componentName == "$packageName/$aliasTargetActivity" || it.aliasTarget == aliasTargetActivity
            }

            // We skip this alias if there already exists one that covers the same actions and categories.
            if (existingTargets.any {
                    it.actions.containsAll(intentReceiver.actions) && it.categories.containsAll(intentReceiver.categories)
                }) {
                println("Skipping alias: ${intentReceiver.componentName} with target $aliasTargetActivity")
                continue
            }
        }

        // Skip duplicate intent receivers (i.e., all properties are the same).
        // Duplicates happen because an activity can declare multiple intent filters.
        // They may have different <data> tags, which we do not parse (yet?).
        // Might be helpful to use them since they hint at the scheme and host of the URI.
        if (targets.contains(intentReceiver)) {
            println("Skipping duplicate intent receiver: ${intentReceiver.componentName}")
            continue
        }

        println("Adding intent receiver: ${intentReceiver.componentName}")
        targets.add(intentReceiver)
    }

    return targets
}
