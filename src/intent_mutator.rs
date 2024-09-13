//! [Mutator]s for [IntentInput].

use std::marker::PhantomData;

use libafl::{
    prelude::{
        tuple_list, tuple_list_type, BytesInput, HasBytesVec, MutationResult, Mutator, Named, Rand,
        StdScheduledMutator,
    },
    state::{HasCorpus, HasMaxSize, HasNamedMetadata, HasRand},
};
use strum::IntoEnumIterator;

use crate::{
    intent_generator::IntentTemplate,
    intent_input::{
        DirectInput, ExtraInput, ExtraType, IntentInput, MimeType, URIInput, URIScheme, URISuffix,
    },
    util::COMMON_EXTRA_KEYS,
};

/// Mutator that randomly modifies the flags attribute of the intent.
pub struct IntentRandomFlagMutator<S>
where
    S: HasRand,
{
    phantom: PhantomData<S>,
}

impl<S> IntentRandomFlagMutator<S>
where
    S: HasRand,
{
    pub fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<S> Named for IntentRandomFlagMutator<S>
where
    S: HasRand,
{
    fn name(&self) -> &str {
        "IntentRandomFlagMutator"
    }
}

impl<S> Mutator<IntentInput, S> for IntentRandomFlagMutator<S>
where
    S: HasRand,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut IntentInput,
        _stage_idx: i32,
    ) -> Result<libafl::prelude::MutationResult, libafl::Error> {
        let bit = 1 << state.rand_mut().choose(0..8);
        input.flags ^= bit;
        Ok(MutationResult::Mutated)
    }
}

/// Mutator that randomly modifies the data attribute of the intent.
pub struct IntentRandomDataMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize,
{
    backing_byte_mutator: StdScheduledMutator<BytesInput, BaseByteMutationsType, S>,
}

impl<S> IntentRandomDataMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize,
{
    pub fn new() -> Self {
        Self {
            backing_byte_mutator: StdScheduledMutator::new(base_byte_mutations()),
        }
    }
}

impl<S> Named for IntentRandomDataMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize,
{
    fn name(&self) -> &str {
        "IntentRandomDataMutator"
    }
}

impl<S> Mutator<IntentInput, S> for IntentRandomDataMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut IntentInput,
        stage_idx: i32,
    ) -> Result<libafl::prelude::MutationResult, libafl::Error> {
        // Check if the data is already a byte input
        match &mut input.data {
            Some(uri_input) => match state.rand_mut().between(1, 3) {
                1 => {
                    // Mutate the scheme
                    uri_input.scheme = state.rand_mut().choose(URIScheme::iter());
                }
                2 => {
                    // Mutate the suffix
                    uri_input.suffix = state.rand_mut().choose(URISuffix::iter());
                }
                _ => {
                    // Mutate the content
                    return self.backing_byte_mutator.mutate(
                        state,
                        &mut uri_input.content,
                        stage_idx,
                    );
                }
            },
            None => {
                let mut uri_input = URIInput {
                    scheme: state.rand_mut().choose(URIScheme::iter()),
                    suffix: state.rand_mut().choose(URISuffix::iter()),
                    content: BytesInput::new(Vec::new()),
                };

                let result =
                    self.backing_byte_mutator
                        .mutate(state, &mut uri_input.content, stage_idx);

                input.data.get_or_insert(uri_input);

                return result;
            }
        }

        Ok(MutationResult::Mutated)
    }
}

/// Mutator that modifies the type attribute of the intent.
pub struct IntentRandomMimeTypeMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize,
{
    phantom: PhantomData<S>,
}

impl<S> IntentRandomMimeTypeMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize,
{
    pub fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<S> Named for IntentRandomMimeTypeMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize,
{
    fn name(&self) -> &str {
        "IntentRandomTypeMutator"
    }
}

impl<S> Mutator<IntentInput, S> for IntentRandomMimeTypeMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut IntentInput,
        _stage_idx: i32,
    ) -> Result<libafl::prelude::MutationResult, libafl::Error> {
        // Choose a random mimetype from the enum.
        input.mime_type = state.rand_mut().choose(MimeType::iter());
        Ok(MutationResult::Mutated)
    }
}

// Mutator that randomly modifies the key attribute of the extra.
pub struct IntentRandomAddExtraMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    backing_byte_mutator: StdScheduledMutator<BytesInput, BaseByteMutationsType, S>,
}

impl<S> Named for IntentRandomAddExtraMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    fn name(&self) -> &str {
        "IntentRandomAddExtraMutator"
    }
}

impl<S> IntentRandomAddExtraMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    pub fn new() -> Self {
        Self {
            backing_byte_mutator: StdScheduledMutator::new(base_byte_mutations()),
        }
    }
}

impl<S> Mutator<IntentInput, S> for IntentRandomAddExtraMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut IntentInput,
        stage_idx: i32,
    ) -> Result<libafl::prelude::MutationResult, libafl::Error> {
        if input.extras.len() >= 10 {
            return Ok(MutationResult::Skipped);
        }

        input.extras.push(generate_random_extra(state));

        let extra: &mut ExtraInput = &mut input.extras.last_mut().unwrap();

        // Mutate the content
        mutate_content(&mut self.backing_byte_mutator, state, extra, stage_idx)
    }
}

// Mutator that randomly modifies the key attribute of the extra.
pub struct IntentRandomExtraKeyMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    phantom: PhantomData<S>,
}

impl<S> Named for IntentRandomExtraKeyMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    fn name(&self) -> &str {
        "IntentRandomExtraKeyMutator"
    }
}

impl<S> IntentRandomExtraKeyMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    pub fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<S> Mutator<IntentInput, S> for IntentRandomExtraKeyMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut IntentInput,
        _stage_idx: i32,
    ) -> Result<libafl::prelude::MutationResult, libafl::Error> {
        let extra = match get_extra_to_mutate(state, input) {
            Ok(extra) => extra,
            Err(_) => return Ok(MutationResult::Skipped),
        };

        // Mutate the key
        let intent_template = state
            .named_metadata::<IntentTemplate>("intent_template")
            .expect("Missing intent template")
            .clone();

        let extras_keys: Vec<&str> = intent_template
            .known_extras_keys
            .keys()
            .map(String::as_str)
            .chain(COMMON_EXTRA_KEYS.iter().map(|s| s.0))
            .collect();

        let extra_key = state.rand_mut().choose(extras_keys);

        extra.key = extra_key.to_owned();

        Ok(MutationResult::Mutated)
    }
}

// Mutator that randomly modifies the content attribute of the extra.
pub struct IntentRandomExtraContentMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    backing_byte_mutator: StdScheduledMutator<BytesInput, BaseByteMutationsType, S>,
}

impl<S> Named for IntentRandomExtraContentMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    fn name(&self) -> &str {
        "IntentRandomExtraContentMutator"
    }
}

impl<S> IntentRandomExtraContentMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    pub fn new() -> Self {
        Self {
            backing_byte_mutator: StdScheduledMutator::new(base_byte_mutations()),
        }
    }
}

impl<S> Mutator<IntentInput, S> for IntentRandomExtraContentMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut IntentInput,
        stage_idx: i32,
    ) -> Result<libafl::prelude::MutationResult, libafl::Error> {
        let extra = match get_extra_to_mutate(state, input) {
            Ok(extra) => extra,
            Err(_) => return Ok(MutationResult::Skipped),
        };

        // Mutate the content
        mutate_content(&mut self.backing_byte_mutator, state, extra, stage_idx)
    }
}

// Mutator that randomly modifies the scheme attribute of the extra.
pub struct IntentRandomExtraSchemeMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    phantom: PhantomData<S>,
}

impl<S> Named for IntentRandomExtraSchemeMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    fn name(&self) -> &str {
        "IntentRandomExtraSchemeMutator"
    }
}

impl<S> IntentRandomExtraSchemeMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    pub fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<S> Mutator<IntentInput, S> for IntentRandomExtraSchemeMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut IntentInput,
        _stage_idx: i32,
    ) -> Result<libafl::prelude::MutationResult, libafl::Error> {
        let extra = match get_extra_to_mutate(state, input) {
            Ok(extra) => extra,
            Err(_) => return Ok(MutationResult::Skipped),
        };

        // Mutate the scheme
        Ok(match &mut extra.value {
            ExtraType::URI(uri) => {
                uri.scheme = state.rand_mut().choose(URIScheme::iter());
                MutationResult::Mutated
            }
            _ => MutationResult::Skipped,
        })
    }
}

// Mutator that randomly modifies the suffix attribute of the extra.
pub struct IntentRandomExtraSuffixMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    phantom: PhantomData<S>,
}

impl<S> Named for IntentRandomExtraSuffixMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    fn name(&self) -> &str {
        "IntentRandomExtraSuffixMutator"
    }
}

impl<S> IntentRandomExtraSuffixMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    pub fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<S> Mutator<IntentInput, S> for IntentRandomExtraSuffixMutator<S>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut IntentInput,
        _stage_idx: i32,
    ) -> Result<libafl::prelude::MutationResult, libafl::Error> {
        let extra = match get_extra_to_mutate(state, input) {
            Ok(extra) => extra,
            Err(_) => return Ok(MutationResult::Skipped),
        };

        // Mutate the suffix
        Ok(match &mut extra.value {
            ExtraType::URI(uri) => {
                uri.suffix = state.rand_mut().choose(URISuffix::iter());
                MutationResult::Mutated
            }
            _ => MutationResult::Skipped,
        })
    }
}

// -----------------------------------------

/// Helper function to get an ExtraInput to mutate. Creates a new one if there
/// are no extras yet.
fn get_extra_to_mutate<'a, S>(
    state: &mut S,
    input: &'a mut IntentInput,
) -> Result<&'a mut ExtraInput, libafl::Error>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    if input.extras.is_empty() {
        // Add a new extra.
        return Err(libafl::Error::unknown("No extras to mutate"));
    }

    // Mutate one extra value.
    let index = state.rand_mut().between(0, (input.extras.len() - 1) as u64) as usize;

    Ok(input.extras.get_mut(index).unwrap())
}

/// Helper function to get a random ExtraInput.
fn generate_random_extra<S>(state: &mut S) -> ExtraInput
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    // Choose an extra from the template in the state.
    let intent_template = state
        .named_metadata::<IntentTemplate>("intent_template")
        .expect("Missing intent template")
        .clone();

    // Get a random key and its type from the template.
    let combined_iterator = intent_template
        .known_extras_keys
        .iter()
        .map(|(key, extra_type)| (key.as_str(), extra_type.as_str()))
        .chain(COMMON_EXTRA_KEYS)
        .collect::<Vec<(&str, &str)>>();

    let (key, extra_type) = state.rand_mut().choose(combined_iterator);

    //println!("Generating extra with key {} and type {}", key, extra_type);

    // Create an extra with the key and a random value.
    let extra = match extra_type {
        "Boolean" => ExtraType::Boolean(DirectInput {
            buffer: BytesInput::new(Vec::new()),
        }),
        "Float" => ExtraType::Float(DirectInput {
            buffer: BytesInput::new(Vec::new()),
        }),
        "Int" => ExtraType::Int(DirectInput {
            buffer: BytesInput::new(Vec::new()),
        }),
        "Long" => ExtraType::Long(DirectInput {
            buffer: BytesInput::new(Vec::new()),
        }),
        "String" => ExtraType::String(DirectInput {
            buffer: BytesInput::new(Vec::new()),
        }),
        "URI" => ExtraType::URI(URIInput {
            scheme: state.rand_mut().choose(URIScheme::iter()),
            suffix: state.rand_mut().choose(URISuffix::iter()),
            content: BytesInput::new(Vec::new()),
        }),
        "ComponentName" => ExtraType::ComponentName(DirectInput {
            buffer: BytesInput::new(Vec::new()),
        }),
        "IntArray" => ExtraType::IntArray(DirectInput {
            buffer: BytesInput::new(Vec::new()),
        }),
        "IntArrayList" => ExtraType::IntArrayList(DirectInput {
            buffer: BytesInput::new(Vec::new()),
        }),
        "LongArray" => ExtraType::LongArray(DirectInput {
            buffer: BytesInput::new(Vec::new()),
        }),
        "LongArrayList" => ExtraType::LongArrayList(DirectInput {
            buffer: BytesInput::new(Vec::new()),
        }),
        "FloatArray" => ExtraType::FloatArray(DirectInput {
            buffer: BytesInput::new(Vec::new()),
        }),
        "FloatArrayList" => ExtraType::FloatArrayList(DirectInput {
            buffer: BytesInput::new(Vec::new()),
        }),
        "StringArray" => ExtraType::StringArray(DirectInput {
            buffer: BytesInput::new(Vec::new()),
        }),
        "StringArrayList" => ExtraType::StringArrayList(DirectInput {
            buffer: BytesInput::new(Vec::new()),
        }),
        _ => ExtraType::Boolean(DirectInput {
            // TODO: Implement me
            buffer: BytesInput::new(Vec::new()),
        }),
    };

    ExtraInput {
        key: key.to_owned(),
        value: extra,
    }
}

fn mutate_content<S>(
    mutator: &mut StdScheduledMutator<BytesInput, BaseByteMutationsType, S>,
    state: &mut S,
    extra: &mut ExtraInput,
    stage_idx: i32,
) -> Result<MutationResult, libafl::Error>
where
    S: HasRand + HasCorpus + HasMaxSize + HasNamedMetadata,
{
    let result = mutator.mutate(state, &mut extra.value.content_buffer(), stage_idx);

    // If the mutation was successful, resize the extra value to the correct size.
    if let Ok(MutationResult::Mutated) = result {
        match &mut extra.value {
            ExtraType::Boolean(value) => value.buffer.bytes_mut().resize(1, 0),
            ExtraType::Int(value) | ExtraType::Float(value) => {
                value.buffer.bytes_mut().resize(4, 0)
            }
            ExtraType::Long(value) => value.buffer.bytes_mut().resize(8, 0),
            _ => {}
        }
    }

    result
}

/// This is basically a copy of <https://github.com/AFLplusplus/LibAFL/blob/8f8e74d670b3aadda6b288b6f1a2de8a1cf98379/libafl/src/mutators/scheduled.rs#L204>
/// but without the crossover mutations which require the corpus to be a
/// BytesInput.
type BaseByteMutationsType = tuple_list_type!(
    libafl::prelude::BitFlipMutator,
    libafl::prelude::ByteIncMutator,
    libafl::prelude::ByteDecMutator,
    libafl::prelude::ByteNegMutator,
    libafl::prelude::ByteRandMutator,
    libafl::prelude::ByteAddMutator,
    libafl::prelude::WordAddMutator,
    libafl::prelude::DwordAddMutator,
    libafl::prelude::QwordAddMutator,
    libafl::prelude::ByteInterestingMutator,
    libafl::prelude::WordInterestingMutator,
    libafl::prelude::DwordInterestingMutator,
    libafl::prelude::BytesDeleteMutator,
    libafl::prelude::BytesDeleteMutator,
    libafl::prelude::BytesDeleteMutator,
    libafl::prelude::BytesDeleteMutator,
    libafl::prelude::BytesExpandMutator,
    libafl::prelude::BytesInsertMutator,
    libafl::prelude::BytesRandInsertMutator,
    libafl::prelude::BytesSetMutator,
    libafl::prelude::BytesRandSetMutator,
    libafl::prelude::BytesCopyMutator,
    libafl::prelude::BytesInsertCopyMutator,
    libafl::prelude::BytesSwapMutator,
);

fn base_byte_mutations() -> BaseByteMutationsType {
    tuple_list!(
        libafl::prelude::BitFlipMutator::new(),
        libafl::prelude::ByteIncMutator::new(),
        libafl::prelude::ByteDecMutator::new(),
        libafl::prelude::ByteNegMutator::new(),
        libafl::prelude::ByteRandMutator::new(),
        libafl::prelude::ByteAddMutator::new(),
        libafl::prelude::WordAddMutator::new(),
        libafl::prelude::DwordAddMutator::new(),
        libafl::prelude::QwordAddMutator::new(),
        libafl::prelude::ByteInterestingMutator::new(),
        libafl::prelude::WordInterestingMutator::new(),
        libafl::prelude::DwordInterestingMutator::new(),
        libafl::prelude::BytesDeleteMutator::new(),
        libafl::prelude::BytesDeleteMutator::new(),
        libafl::prelude::BytesDeleteMutator::new(),
        libafl::prelude::BytesDeleteMutator::new(),
        libafl::prelude::BytesExpandMutator::new(),
        libafl::prelude::BytesInsertMutator::new(),
        libafl::prelude::BytesRandInsertMutator::new(),
        libafl::prelude::BytesSetMutator::new(),
        libafl::prelude::BytesRandSetMutator::new(),
        libafl::prelude::BytesCopyMutator::new(),
        libafl::prelude::BytesInsertCopyMutator::new(),
        libafl::prelude::BytesSwapMutator::new(),
    )
}
