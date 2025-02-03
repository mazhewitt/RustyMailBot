#!/usr/bin/env python

import torch
from transformers import AutoTokenizer, AutoModel
import torch.nn as nn

class SentenceEmbeddingModel(nn.Module):
    """
    A simple wrapper that loads a transformer model and performs mean pooling.
    This version assumes the model outputs a last_hidden_state.
    """
    def __init__(self, model_name: str):
        super().__init__()
        self.model = AutoModel.from_pretrained(model_name)

    def forward(self, input_ids: torch.Tensor, attention_mask: torch.Tensor) -> torch.Tensor:
        outputs = self.model(input_ids=input_ids, attention_mask=attention_mask)
        # last_hidden_state shape: [batch_size, seq_len, hidden_size]
        token_embeddings = outputs.last_hidden_state
        # Expand the attention mask so it can be multiplied with the embeddings.
        input_mask_expanded = attention_mask.unsqueeze(-1).expand(token_embeddings.size()).float()
        # Compute the sum of the embeddings, taking the mask into account.
        sum_embeddings = torch.sum(token_embeddings * input_mask_expanded, dim=1)
        # Avoid division by zero.
        sum_mask = torch.clamp(input_mask_expanded.sum(dim=1), min=1e-9)
        mean_embeddings = sum_embeddings / sum_mask
        return mean_embeddings

def main():
    # Specify the model name from Hugging Face.
    model_name = "./data/models/all-MiniLM-L12-v2"

    # Load the tokenizer.
    tokenizer = AutoTokenizer.from_pretrained(model_name)

    # Initialize the embedding model.
    model = SentenceEmbeddingModel(model_name)
    model.eval()  # set to evaluation mode

    # Prepare a sample input for tracing.
    sample_text = "This is a test sentence."
    encoded = tokenizer(sample_text, return_tensors="pt", padding=True, truncation=True)
    input_ids = encoded["input_ids"]
    attention_mask = encoded["attention_mask"]

    # Trace the model using the sample input.
    traced_model = torch.jit.trace(model, (input_ids, attention_mask))

    # Save the traced model in TorchScript format.
    output_file = "rust_model.ot"
    traced_model.save(output_file)
    print(f"TorchScript model saved to {output_file}")

if __name__ == "__main__":
    main()